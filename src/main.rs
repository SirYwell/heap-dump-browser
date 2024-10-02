mod load_file;
mod view_heap_dump;

use crate::load_file::UploadFile;
use crate::view_heap_dump::ViewHeapDump;
use patternfly_yew::prelude::{BackdropViewer, ToastViewer};
use yew::prelude::*;
use yew_nested_router::{Router, Switch, Target};

#[function_component]
fn App() -> Html {
    html! {
        <BackdropViewer>
            <ToastViewer>
                <Router<AppRoute> default={AppRoute::Upload}>
                    <Switch<AppRoute> render={route} />
                </Router<AppRoute>>
            </ToastViewer>
        </BackdropViewer>
    }}

#[derive(Debug, Default, Clone, PartialEq, Eq, Target)]
enum AppRoute {
    #[default]
    Upload,
    Analysis
}

fn route(target: AppRoute) -> Html {
    match target {
        AppRoute::Upload => html!(<UploadFile/>),
        AppRoute::Analysis => html!(<ViewHeapDump/>)
    }
}

#[cfg(not(debug_assertions))]
const LOG_LEVEL: log::Level = log::Level::Info;
#[cfg(debug_assertions)]
const LOG_LEVEL: log::Level = log::Level::Trace;

fn main() {
    wasm_logger::init(wasm_logger::Config::new(LOG_LEVEL));
    yew::Renderer::<App>::new().render();
}
