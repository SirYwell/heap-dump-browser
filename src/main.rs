mod load_file;
mod view_heap_dump;
mod heap_dump;

use crate::load_file::UploadFile;
use crate::view_heap_dump::ViewHeapDump;
use patternfly_yew::prelude::{BackdropViewer, ToastViewer};
use yew::prelude::*;
use yew_router::{BrowserRouter, Routable, Switch};

#[function_component]
fn App() -> Html {
    html! {
        <BackdropViewer>
            <ToastViewer>
                <BrowserRouter>
                    <Switch<AppRoute> render={route} />
                </BrowserRouter>
            </ToastViewer>
        </BackdropViewer>
    }
}

#[derive(Debug, Default, Clone, PartialEq, Routable)]
enum AppRoute {
    #[default]
    #[at("/")]
    Upload,
    #[at("/view")]
    Analysis,
}

fn route(target: AppRoute) -> Html {
    match target {
        AppRoute::Upload => html!(<UploadFile/>),
        AppRoute::Analysis => html!(<ViewHeapDump/>),
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
