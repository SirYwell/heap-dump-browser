use crate::heap_dump::HeapDump;
use patternfly_yew::prelude::{
    use_table_data, Cell, CellContext, MemoizedTableModel, Navigation, Pagination,
    PaginationPosition, Table, TableColumn, TableEntryRenderer, TableHeader, TableMode, Toolbar,
    ToolbarContent, ToolbarItem, ToolbarItemType, UseTableData,
};
use std::rc::Rc;
use yew::html::IntoPropValue;
use yew::{
    function_component, html, html_nested, use_callback, use_memo, use_state_eq,
    Html, Properties,
};
use yew_router::hooks::use_location;
use yew_router::prelude::use_navigator;
use crate::AppRoute;

#[function_component(ViewHeapDump)]
pub(crate) fn view() -> Html {
    let str: String = "<< no name >>".to_string();
    let loc = use_location().unwrap();
    let navigator = use_navigator().unwrap();
    if let Some(state) = loc.state::<HeapDump>() {
        html!(
            <>
            { "This heap dump was created at " }
            { state.created_at.format("%Y-%m-%d %H:%M:%S").to_string() }
            <ClassList heap_dump={state}/>
            </>
        )
    } else {
        log::info!("redirecting to / as no state is present");
        navigator.replace(&AppRoute::Upload);
        html!(<></>)
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum Columns {
    ClassName,
    InstanceCount,
}

#[derive(Clone)]
struct TableEntry(String, usize);

impl TableEntryRenderer<Columns> for TableEntry {
    fn render_cell(&self, context: CellContext<'_, Columns>) -> Cell {
        match context.column {
            Columns::ClassName => html!({ self.0.clone() }),
            Columns::InstanceCount => html!(self.1),
        }
        .into()
    }
}

#[derive(PartialEq, Properties)]
struct Props {
    heap_dump: Rc<HeapDump>,
}

#[function_component(ClassList)]
fn class_list(props: &Props) -> Html {
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
                TableEntry(
                    props
                        .heap_dump
                        .names
                        .get(&class_info.class_name_id)
                        .unwrap()
                        .clone(),
                    props
                        .heap_dump
                        .objects_by_class
                        .get(&class_info.class_object_id)
                        .map(|v| v.len())
                        .unwrap_or(0),
                )
            })
            .collect::<Vec<_>>()
    });

    let (entries, _) = use_table_data(MemoizedTableModel::new(entries));

    let header = html_nested! {
        <TableHeader<Columns>>
            <TableColumn<Columns> label="Class Name" index={Columns::ClassName} />
            <TableColumn<Columns> label="Instance Count" index={Columns::InstanceCount} />
        </TableHeader<Columns>>
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
            <Table<Columns, UseTableData<Columns, MemoizedTableModel<TableEntry>>>
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
