use crate::heap_dump::{AnalysisClassInfo, HeapDump, InstanceInfo, Reference, FAKE_ROOT_ID};
use crate::AppRoute;
use hprof_rs::hprof_model::U8;
use itertools::Itertools;
use mini_moka::unsync::Cache;
use patternfly_yew::prelude::{
    use_table_data, Cell, CellContext, MemoizedTableModel, Navigation, Pagination,
    PaginationPosition, Tab, Table, TableColumn, TableEntryRenderer, TableHeader, TableMode, Tabs,
    Toolbar, ToolbarContent, ToolbarItem, ToolbarItemType, UseTableData,
};
use petgraph::algo;
use std::collections::HashMap;
use std::rc::Rc;
use yew::function_component;
use yew::html;
use yew::html_nested;
use yew::use_callback;
use yew::use_memo;
use yew::use_state_eq;
use yew::Html;
use yew::Properties;
use yew_router::hooks::use_location;
use yew_router::prelude::use_navigator;
use yew_router::Routable;

#[derive(Default, Clone, PartialEq, Routable)]
enum AnalysisRoutes {
    #[default]
    #[at("/view")]
    Overview,
    #[at("/view/plugins")]
    Plugins,
}

#[function_component(ViewHeapDump)]
pub(crate) fn view() -> Html {
    let loc = use_location().unwrap();
    let navigator = use_navigator().unwrap();

    let selected = use_state_eq(|| 1);
    let onselect = use_callback(selected.clone(), |index, selected| selected.set(index));

    if let Some(state) = loc.state::<HeapDump>() {
        html!(
            <>
            <Tabs<usize> selected={*selected} {onselect}>
                <Tab<usize> index=1 title="Overview">
                    { "This heap dump was created at " }
                    { state.created_at.format("%Y-%m-%d %H:%M:%S").to_string() }
                    <ClassTable heap_dump={state.clone()}/>
                </Tab<usize>>
                <Tab<usize> index=2 title="Plugins">
                    <PluginTable heap_dump={state.clone()}/>
                </Tab<usize>>
                <Tab<usize> index=4 title="Memory Usage Bugs">
                    <MemoryBugs heap_dump={state.clone()}/>
                </Tab<usize>>
            </Tabs<usize>>
        </>
        )
    } else {
        log::info!("redirecting to / as no state is present");
        navigator.replace(&AppRoute::Upload);
        html!(<></>)
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum ClassTableColumns {
    ClassName,
    InstanceCount,
}

#[derive(Clone)]
struct ClassTableEntry(String, usize);

impl TableEntryRenderer<ClassTableColumns> for ClassTableEntry {
    fn render_cell(&self, context: CellContext<'_, ClassTableColumns>) -> Cell {
        match context.column {
            ClassTableColumns::ClassName => html!({ self.0.clone() }),
            ClassTableColumns::InstanceCount => html!(self.1),
        }
        .into()
    }
}

#[derive(PartialEq, Properties)]
struct Props {
    heap_dump: Rc<HeapDump>,
}

#[function_component(ClassTable)]
fn class_table(props: &Props) -> Html {
    let offset = use_state_eq(|| 0);
    let limit = use_state_eq(|| 5);

    let size = props.heap_dump.classes.len().clone();

    let entries = use_memo((*offset, *limit), |(offset, limit)| {
        props
            .heap_dump
            .classes
            .values()
            .skip(*offset)
            .take(*limit)
            .map(|class_info| {
                ClassTableEntry(
                    props
                        .heap_dump
                        .names
                        .get(&class_info.class_name_id)
                        .unwrap()
                        .clone(),
                    props
                        .heap_dump
                        .objects_by_class
                        .get_vec(&class_info.class_object_id)
                        .map(|v| v.len())
                        .unwrap_or(0),
                )
            })
            .collect::<Vec<_>>()
    });

    let (entries, _) = use_table_data(MemoizedTableModel::new(entries));

    let header = html_nested! {
        <TableHeader<ClassTableColumns >>
            <TableColumn<ClassTableColumns> label="Class Name" index={ClassTableColumns::ClassName} />
            <TableColumn<ClassTableColumns> label="Instance Count" index={ClassTableColumns::InstanceCount} />
        </TableHeader<ClassTableColumns >>
    };

    let total_entries = Some(size);

    let limit_callback = use_callback(limit.clone(), |number, limit| limit.set(number));
    let s = size;
    let nav_callback = use_callback(
        (offset.clone(), *limit),
        move |page: Navigation, (offset, limit)| {
            let o = match page {
                Navigation::First => 0,
                Navigation::Last => ((s - 1) / limit) * limit,
                Navigation::Previous => **offset - limit,
                Navigation::Next => **offset + limit,
                Navigation::Page(n) => n * limit,
            };
            offset.set(o);
        },
    );

    html! (
        <>
            <Toolbar>
                <ToolbarContent>
                    <ToolbarItem r#type={ToolbarItemType::Pagination}>
                        <Pagination
                            {total_entries}
                            offset={*offset}
                            entries_per_page_choices={vec![5, 10, 25, 50, 100]}
                            selected_choice={*limit}
                            onlimit={&limit_callback}
                            onnavigation={&nav_callback}
                        />
                    </ToolbarItem>
                </ToolbarContent>
            </Toolbar>
            <Table<ClassTableColumns, UseTableData<ClassTableColumns, MemoizedTableModel<ClassTableEntry >>>
                mode={TableMode::Compact}
                {header}
                {entries}
            />
            <Pagination
                {total_entries}
                offset={*offset}
                entries_per_page_choices={vec![5, 10, 25, 50, 100]}
                selected_choice={*limit}
                onlimit={&limit_callback}
                onnavigation={&nav_callback}
                position={PaginationPosition::Bottom}
            />
        </>
    )
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum PluginTableColumns {
    MainClassName,
    ClassCount,
}

#[derive(Clone)]
struct PluginTableEntry(String, usize);

impl TableEntryRenderer<PluginTableColumns> for PluginTableEntry {
    fn render_cell(&self, context: CellContext<'_, PluginTableColumns>) -> Cell {
        match context.column {
            PluginTableColumns::MainClassName => html!({ self.0.clone() }),
            PluginTableColumns::ClassCount => html!(self.1),
        }
        .into()
    }
}

#[function_component(PluginTable)]
fn plugin_table(props: &Props) -> Html {
    let mut is_plugin_class_cache: Cache<U8, bool> = Cache::builder().max_capacity(512).build();
    let classes = &props.heap_dump.classes;
    let names = &props.heap_dump.names;
    let plugin_instances = props
        .heap_dump
        .objects
        .values()
        .filter_map(|reference| match &**reference {
            Reference::Instance(instance) => Some(instance),
            Reference::ObjectArray(_) => None,
            Reference::PrimitiveArray(_) => None,
            Reference::FakeCommonRoot => None,
        })
        .filter(|instance| is_plugin_class(instance, &mut is_plugin_class_cache, classes, names))
        .collect::<Vec<_>>();

    let by_loader = classes
        .values()
        .into_grouping_map_by(|class| class.class_loader_object_id)
        .collect::<Vec<_>>();

    let offset = use_state_eq(|| 0);
    let limit = use_state_eq(|| 5);

    let size = plugin_instances.len();

    let entries = use_memo((*offset, *limit), |(offset, limit)| {
        plugin_instances
            .iter()
            .skip(*offset)
            .take(*limit)
            .filter_map(|instance| classes.get(&instance.class_object_id))
            .map(|class_info| {
                PluginTableEntry(
                    names.get(&class_info.class_name_id).unwrap().clone(),
                    by_loader
                        .get(&class_info.class_loader_object_id)
                        .map(|v| v.len())
                        .unwrap_or(0),
                )
            })
            .collect::<Vec<_>>()
    });

    let (entries, _) = use_table_data(MemoizedTableModel::new(entries));

    let header = html_nested! {
        <TableHeader<PluginTableColumns>>
            <TableColumn<PluginTableColumns> label="Main Class" index={PluginTableColumns::MainClassName} />
            <TableColumn<PluginTableColumns> label="Loaded Classes (by same classloader)" index={PluginTableColumns::ClassCount} />
        </TableHeader<PluginTableColumns>>
    };

    let total_entries = Some(size);

    let limit_callback = use_callback(limit.clone(), |number, limit| limit.set(number));
    let s = size;
    let nav_callback = use_callback(
        (offset.clone(), *limit),
        move |page: Navigation, (offset, limit)| {
            let o = match page {
                Navigation::First => 0,
                Navigation::Last => ((s - 1) / limit) * limit,
                Navigation::Previous => **offset - limit,
                Navigation::Next => **offset + limit,
                Navigation::Page(n) => n * limit,
            };
            offset.set(o);
        },
    );

    html! (
        <>
            <Toolbar>
                <ToolbarContent>
                    <ToolbarItem r#type={ToolbarItemType::Pagination}>
                        <Pagination
                            {total_entries}
                            offset={*offset}
                            entries_per_page_choices={vec![5, 10, 25, 50, 100]}
                            selected_choice={*limit}
                            onlimit={&limit_callback}
                            onnavigation={&nav_callback}
                        />
                    </ToolbarItem>
                </ToolbarContent>
            </Toolbar>
            <Table<PluginTableColumns, UseTableData<PluginTableColumns, MemoizedTableModel<PluginTableEntry>>>
                mode={TableMode::Compact}
                {header}
                {entries}
            />
            <Pagination
                {total_entries}
                offset={*offset}
                entries_per_page_choices={vec![5, 10, 25, 50, 100]}
                selected_choice={*limit}
                onlimit={&limit_callback}
                onnavigation={&nav_callback}
                position={PaginationPosition::Bottom}
            />
        </>
    )
}

fn is_plugin_class(
    instance_info: &InstanceInfo,
    is_plugin_class_cache: &mut Cache<U8, bool>,
    classes: &HashMap<U8, AnalysisClassInfo>,
    names: &HashMap<U8, String>,
) -> bool {
    let mut class_id = &instance_info.class_object_id;

    if let Some(b) = is_plugin_class_cache.get(class_id) {
        *b // cache hit
    } else {
        // collect all superclasses in this stack
        let mut stack = Vec::new();
        while let Some(class_info) = classes.get(class_id) {
            stack.push(*class_id);
            if is_java_plugin_class_exact(names, &class_info) {
                for x in stack {
                    is_plugin_class_cache.insert(x, true);
                }
                return true;
            }
            class_id = &class_info.super_class_object_id;
            if let Some(&b) = is_plugin_class_cache.get(class_id) {
                // found superclass in cache, remember for all seen classes
                for x in stack {
                    is_plugin_class_cache.insert(x, b);
                }
                return b;
            }
        }
        for x in stack {
            is_plugin_class_cache.insert(x, false);
        }
        false
    }
}

fn is_java_plugin_class_exact(names: &HashMap<U8, String>, class_info: &AnalysisClassInfo) -> bool {
    names
        .get(&class_info.class_name_id)
        .unwrap_or(&"<< no name >>".to_string())
        == "org/bukkit/plugin/java/JavaPlugin"
}

fn is_craft_player_class(names: &HashMap<U8, String>, class_info: &AnalysisClassInfo) -> bool {
    let no_name = "<< no name >>".to_string();
    let name = names.get(&class_info.class_name_id).unwrap_or(&no_name);
    name == "org/bukkit/craftbukkit/entity/CraftPlayer"
}

#[function_component(MemoryBugs)]
fn memory_bugs(props: &Props) -> Html {
    let heap_dump = &props.heap_dump;
    let names = &props.heap_dump.names;
    log::info!("number of classes: {}", &props.heap_dump.classes.len());
    let class = &props
        .heap_dump
        .classes
        .values()
        .find(|class| is_craft_player_class(names, class))
        .expect("CraftPlayer class not loaded?");

    let object_graph = &props.heap_dump.object_graph;

    let empty_vec = Vec::new();
    let player_instances = props
        .heap_dump
        .objects_by_class
        .get_vec(&class.class_object_id)
        .unwrap_or(&empty_vec);

    let rc = FAKE_ROOT_ID;
    let mut leaking_instances = Vec::new();
    for player_instance in player_instances {
        let x = match &**player_instance {
            Reference::Instance(instance_info) => instance_info,
            _ => panic!("player instance is not an instance"),
        };
        let mut paths =
            algo::all_simple_paths::<Vec<_>, _>(object_graph, rc.clone(), x.object_id, 1, None);
        let referenced_by_mc = paths.all(|path| {
            path.iter().any(|reference| {
                let reference = heap_dump.objects.get(reference);
                match reference {
                    Some(thing) => {
                        let x = &**thing;
                        match x {
                            Reference::Instance(instance_info) => {
                                is_loaded_by_mc(&instance_info, &heap_dump)
                            }
                            Reference::ObjectArray(_) => false, // TODO who loads array classes?
                            _ => false,
                        }
                    }
                    None => false,
                }
            })
        });
        if !referenced_by_mc {
            leaking_instances.push(player_instance.clone())
        }
    }

    html!(<>{leaking_instances.len()}</>)
}

fn is_loaded_by_mc(instance_info: &InstanceInfo, heap_dump: &HeapDump) -> bool {
    let class_id = instance_info.class_object_id;
    let class: Option<&AnalysisClassInfo> = heap_dump.classes.get(&class_id);
    class.filter(|c| c.class_loader_object_id != 0u64).is_some() // TODO add (other?) allowed classloaders
}
