use chrono::{DateTime, Utc};
use hprof_rs::hprof_model::{HeapDumpTag, RecordTag, Value, U8};
use hprof_rs::reader::HprofReader;
use multimap::MultiMap;
use petgraph::graphmap::DiGraphMap;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io::{Read, Seek};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub const FAKE_ROOT_ID: U8 = U8::MAX; 

pub struct HeapDump {
    id: u64,
    pub created_at: DateTime<Utc>,
    pub names: HashMap<U8, String>,
    pub classes: HashMap<U8, AnalysisClassInfo>,
    pub objects: HashMap<U8, Rc<Reference>>,
    pub objects_by_class: MultiMap<U8, Rc<Reference>>,
    pub object_graph: DiGraphMap<U8, ()>,
}

impl HeapDump {
    fn new(
        created_at: DateTime<Utc>,
        names: HashMap<U8, String>,
        classes: HashMap<U8, AnalysisClassInfo>,
        objects: HashMap<U8, Rc<Reference>>,
        objects_by_class: MultiMap<U8, Rc<Reference>>,
        roots: Vec<U8>,
    ) -> HeapDump {
        let mut object_graph: DiGraphMap<U8, ()> =
            DiGraphMap::with_capacity(objects.len(), objects.len() * 2);

        for object in objects.values() {
            match &**object {
                Reference::Instance(instance) => {
                    for value in &instance.fields {
                        match value {
                            Value::Object { object_id } => {
                                object_graph.add_edge(instance.object_id, *object_id, ());
                            }
                            Value::Array { object_id } => {
                                object_graph.add_edge(instance.object_id, *object_id, ());
                            }
                            Value::Byte(_) => {}
                            Value::Char(_) => {}
                            Value::Short(_) => {}
                            Value::Float(_) => {}
                            Value::Double(_) => {}
                            Value::Int(_) => {}
                            Value::Long(_) => {}
                            Value::Boolean(_) => {}
                        }
                    }
                }
                Reference::ObjectArray(array) => {
                    for object_id in &array.values {
                        object_graph.add_edge(array.object_id, *object_id, ());
                    }
                }
                Reference::PrimitiveArray(_) => {} // has no outgoing references
                Reference::FakeCommonRoot => panic!("unexpected fake root")
            }
        }

        for root in roots {
            object_graph.add_edge(FAKE_ROOT_ID, root, ());
        }

        HeapDump {
            id: COUNTER.fetch_add(1, Ordering::AcqRel),
            created_at,
            names,
            classes,
            objects,
            objects_by_class,
            object_graph,
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

#[derive(Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Reference {
    Instance(InstanceInfo),
    ObjectArray(ObjectArray),
    PrimitiveArray(PrimitiveArray),
    /// used to have one common "object" that references
    /// all actual root objects in the object_graph
    FakeCommonRoot,
}

pub struct InstanceInfo {
    pub class_object_id: U8,
    pub object_id: U8,
    pub fields: Vec<Value>,
}

impl Eq for InstanceInfo {}

impl PartialEq<Self> for InstanceInfo {
    fn eq(&self, other: &Self) -> bool {
        self.object_id == other.object_id
    }
}

impl PartialOrd<Self> for InstanceInfo {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.object_id.partial_cmp(&other.object_id)
    }
}

impl Ord for InstanceInfo {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.object_id.cmp(&other.object_id)
    }
}

impl Hash for InstanceInfo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.object_id.hash(state)
    }
}

pub struct ObjectArray {
    pub class_object_id: U8,
    pub object_id: U8,
    pub values: Vec<U8>,
}

impl Eq for ObjectArray {}

impl PartialEq for ObjectArray {
    fn eq(&self, other: &Self) -> bool {
        self.object_id == other.object_id
    }
}

impl PartialOrd for ObjectArray {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.object_id.partial_cmp(&other.object_id)
    }
}

impl Ord for ObjectArray {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.object_id.cmp(&other.object_id)
    }
}

impl Hash for ObjectArray {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.object_id.hash(state)
    }
}

pub struct PrimitiveArray {
    // pub class_object_id: U8, TODO
    pub object_id: U8,
    pub values: Vec<Value>,
}

impl Eq for PrimitiveArray {}

impl PartialEq for PrimitiveArray {
    fn eq(&self, other: &Self) -> bool {
        self.object_id == other.object_id
    }
}

impl PartialOrd for PrimitiveArray {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.object_id.partial_cmp(&other.object_id)
    }
}

impl Ord for PrimitiveArray {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.object_id.cmp(&other.object_id)
    }
}

impl Hash for PrimitiveArray {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.object_id.hash(state)
    }
}

pub fn from_reader<T: Read + Seek>(mut reader: HprofReader<T>) -> HeapDump {
    let mut loaded_classes = HashMap::new();

    let mut names = HashMap::new();
    let mut classes = HashMap::new();
    let mut objects = HashMap::new();
    let mut objects_by_class = MultiMap::new();
    let mut roots = Vec::new();

    while let Some(record) = reader.next() {
        match record {
            Ok(RecordTag::HprofHeapDumpSegment { sub_records, .. }) => {
                for sub_record in sub_records {
                    match sub_record {
                        HeapDumpTag::HprofGcRootUnknown => {}
                        HeapDumpTag::HprofGcRootThreadObj {
                            thread_object_id, ..
                        } => roots.push(thread_object_id),
                        HeapDumpTag::HprofGcRootJniGlobal { object_id, .. } => {
                            roots.push(object_id)
                        }
                        HeapDumpTag::HprofGcRootJniLocal { object_id, .. } => roots.push(object_id),
                        HeapDumpTag::HprofGcRootJavaFrame { object_id, .. } => {
                            roots.push(object_id)
                        }
                        HeapDumpTag::HprofGcRootNativeStack => {}
                        HeapDumpTag::HprofGcRootStickyClass { object_id } => roots.push(object_id),
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
                            let instance = InstanceInfo {
                                fields: instance_field_values,
                                class_object_id,
                                object_id,
                            };
                            let rc = Rc::new(Reference::Instance(instance));
                            objects.insert(object_id, rc.clone());
                            objects_by_class.insert(class_object_id, rc);
                        }
                        HeapDumpTag::HprofGcObjArrayDump {
                            array_object_id,
                            array_class_id,
                            elements,
                            ..
                        } => {
                            let array = ObjectArray {
                                class_object_id: array_class_id,
                                values: elements,
                                object_id: array_object_id,
                            };
                            let rc = Rc::new(Reference::ObjectArray(array));
                            objects.insert(array_object_id, rc.clone());
                            objects_by_class.insert(array_class_id, rc);
                        }
                        HeapDumpTag::HprofGcPrimArrayDump {
                            array_object_id,
                            elements,
                            ..
                        } => {
                            let array = PrimitiveArray {
                                values: elements,
                                object_id: array_object_id,
                            };
                            let rc = Rc::new(Reference::PrimitiveArray(array));
                            objects.insert(array_object_id, rc.clone());
                            // TODO class to object mapping for primitive arrays?
                        }
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
        roots,
    )
}
