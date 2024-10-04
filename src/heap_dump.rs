use chrono::{DateTime, Utc};
use hprof_rs::hprof_model::{HeapDumpTag, RecordTag, Value, U8};
use hprof_rs::reader::HprofReader;
use std::collections::HashMap;
use std::io::{Read, Seek};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct HeapDump {
    id: u64,
    pub created_at: DateTime<Utc>,
    pub names: HashMap<U8, String>,
    pub classes: HashMap<U8, AnalysisClassInfo>,
    pub objects: HashMap<U8, Rc<InstanceInfo>>,
    pub objects_by_class: HashMap<U8, Rc<InstanceInfo>>,
}

impl HeapDump {
    fn new(
        created_at: DateTime<Utc>,
        names: HashMap<U8, String>,
        classes: HashMap<U8, AnalysisClassInfo>,
        objects: HashMap<U8, Rc<InstanceInfo>>,
        objects_by_class: HashMap<U8, Rc<InstanceInfo>>,
    ) -> HeapDump {
        HeapDump {
            id: COUNTER.fetch_add(1, Ordering::AcqRel),
            created_at,
            names,
            classes,
            objects,
            objects_by_class,
        }
    }
}

impl PartialEq for HeapDump {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

pub struct AnalysisClassInfo {
    pub class_object_id: U8,
    pub class_name_id: U8,
    pub super_class_object_id: U8,
    pub class_loader_object_id: U8,
}

pub struct InstanceInfo {
    pub class_object_id: U8,
    pub fields: Vec<Value>,
}

pub fn from_reader<T: Read + Seek>(mut reader: HprofReader<T>) -> HeapDump {
    let mut loaded_classes = HashMap::new();
    let mut classes = HashMap::new();
    let mut names = HashMap::new();
    let mut objects = HashMap::new();
    let mut objects_by_class = HashMap::new();
    while let Some(record) = reader.next() {
        match record {
            Ok(RecordTag::HprofHeapDumpSegment { sub_records, .. }) => {
                for sub_record in sub_records {
                    match sub_record {
                        HeapDumpTag::HprofGcRootUnknown => {}
                        HeapDumpTag::HprofGcRootThreadObj { .. } => {}
                        HeapDumpTag::HprofGcRootJniGlobal { .. } => {}
                        HeapDumpTag::HprofGcRootJniLocal { .. } => {}
                        HeapDumpTag::HprofGcRootJavaFrame { .. } => {}
                        HeapDumpTag::HprofGcRootNativeStack => {}
                        HeapDumpTag::HprofGcRootStickyClass { .. } => {}
                        HeapDumpTag::HprofGcRootThreadBlock => {}
                        HeapDumpTag::HprofGcRootMonitorUsed => {}
                        HeapDumpTag::HprofGcClassDump(class_info) => {
                            let ci = AnalysisClassInfo {
                                class_object_id: class_info.class_object_id,
                                class_name_id: *loaded_classes
                                    .get(&class_info.class_object_id)
                                    .unwrap(),
                                super_class_object_id: class_info.super_class_object_id,
                                class_loader_object_id: class_info.class_loader_object_id,
                            };
                            classes.insert(class_info.class_object_id, ci);
                        }
                        HeapDumpTag::HprofGcInstanceDump {
                            object_id,
                            class_object_id,
                            instance_field_values,
                            ..
                        } => {
                            let rc = Rc::new(InstanceInfo {
                                fields: instance_field_values,
                                class_object_id,
                            });
                            objects.insert(object_id, rc.clone());
                            objects_by_class.insert(class_object_id, rc);
                        }
                        HeapDumpTag::HprofGcObjArrayDump { .. } => {}
                        HeapDumpTag::HprofGcPrimArrayDump { .. } => {}
                    }
                }
            }
            Ok(RecordTag::HprofUtf8 { id, utf8, .. }) => {
                names.insert(id, utf8);
            }
            Ok(RecordTag::HprofLoadClass {
                class_name_id,
                class_object_id,
                ..
            }) => {
                loaded_classes.insert(class_object_id, class_name_id);
            }
            Ok(_) => {}
            Err(_) => {}
        }
    }
    HeapDump::new(
        DateTime::from_timestamp_millis(reader.timestamp as i64).unwrap(),
        names,
        classes,
        objects,
        objects_by_class,
    )
}
