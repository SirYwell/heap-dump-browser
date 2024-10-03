use crate::heap_dump::from_reader;
use crate::AppRoute;
use hprof_rs::reader::HprofReader;
use patternfly_yew::prelude::{
    use_backdrop, Bullseye, Button, ButtonVariant, FileUpload, FileUploadDetails, FileUploadSelect,
    Form, FormGroup, HelperText, HelperTextItem, HelperTextItemVariant, InputGroup, Modal,
    ModalVariant, Spinner, SpinnerSize, TextInput,
};
use std::io::Cursor;
use web_sys::js_sys::{ArrayBuffer, Uint8Array};
use yew::{function_component, html, use_callback, use_node_ref, use_state, Callback, Html};
use yew_hooks::{use_drop_with_options, UseDropOptions};
use yew_more_hooks::hooks::r#async::*;
use yew_router::hooks::use_navigator;

#[derive(Clone, Debug, PartialEq)]
enum DropContent {
    None,
    Files(Vec<web_sys::File>),
    Uri(String),
}

impl DropContent {
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl std::fmt::Display for DropContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Self::Files(files) => {
                for (n, name) in files.iter().map(|f| f.name()).enumerate() {
                    if n > 0 {
                        f.write_str(", ")?;
                    }
                    f.write_str(&name)?;
                }
                Ok(())
            }
            Self::Uri(uri) => f.write_str(uri),
            Self::None => Ok(()),
        }
    }
}

#[function_component(UploadFile)]
pub(crate) fn upload() -> Html {
    let node = use_node_ref();

    let drop_content = use_state(|| DropContent::None);

    let drop = use_drop_with_options(
        node.clone(),
        UseDropOptions {
            onfiles: {
                let drop_content = drop_content.clone();
                Some(Box::new(move |files, _data_transfer| {
                    drop_content.set(DropContent::Files(files));
                }))
            },
            onuri: {
                let drop_content = drop_content.clone();
                Some(Box::new(move |uri, _data_transfer| {
                    drop_content.set(DropContent::Uri(uri));
                }))
            },
            ..Default::default()
        },
    );

    let processing = use_async_with_cloned_deps(
        |content| async move {
            let content = match &*content {
                DropContent::Files(files) => files.first(),
                _ => None,
            };

            match content {
                Some(file) => {
                    // TODO: should that happen on a worker thread?
                    log::trace!("trying to read heap dump");
                    let promise = file.array_buffer();
                    let res = wasm_bindgen_futures::JsFuture::from(promise)
                        .await
                        .map_err(|v| v.as_string().unwrap_or_default())?;
                    let x: ArrayBuffer = ArrayBuffer::from(res.clone());
                    let byte_array = Uint8Array::new(&x);
                    let cursor = Cursor::new(byte_array.to_vec());
                    let r = HprofReader::new(cursor)
                        .map_err(|err| err.to_string())
                        .map(|_| byte_array);
                    log::info!("read heap dump file header");
                    r
                }
                None => Err("Requires a Hprof file".to_string()),
            }
        },
        drop_content.clone(),
    );

    let onclear = use_callback(drop_content.clone(), |_, drop_content| {
        drop_content.set(DropContent::None)
    });

    let error = processing.error().map(|err| err);
    let helper_text = error.clone().map(|err| {
        html!(
            <HelperText live_region=true>
                <HelperTextItem dynamic=true variant={HelperTextItemVariant::Error}>
                    {err}
                </HelperTextItem>
            </HelperText>
        )
    });

    let backdrop = use_backdrop();

    let navigator = use_navigator().unwrap();

    let onsubmit = {
        let processing = processing.clone();
        Callback::from(move |_| {
            if let Some((data, backdrop)) = processing.data().zip(backdrop.as_ref()) {
                log::info!("loading hprof file");
                let cursor = Cursor::new(data.to_vec());
                let r = HprofReader::new(cursor);
                let heap_dump = r.map(|reader| from_reader(reader));
                match heap_dump {
                    Ok(heap_dump) => {
                        backdrop.close();
                        navigator.push_with_state(&AppRoute::Analysis, heap_dump)
                    }
                    Err(err) => backdrop.open(html!(
                        <Bullseye plain=true>
                            <Modal
                                title="Failed to load file"
                                variant={ModalVariant::Large}
                            >
                            {err.to_string()}
                            </Modal>
                        </Bullseye>
                    )),
                }
            }
        })
    };

    let file_input_ref = use_node_ref();
    let onopen = {
        let file_input_ref = file_input_ref.clone();
        Callback::from(move |_| {
            if let Some(ele) = file_input_ref.cast::<web_sys::HtmlElement>() {
                ele.click();
            }
        })
    };

    let onchange_open = {
        let file_input_ref = file_input_ref.clone();
        let drop_content = drop_content.clone();
        Callback::from(move |_| {
            if let Some(ele) = file_input_ref.cast::<web_sys::HtmlInputElement>() {
                let files = ele
                    .files()
                    .map(|files| {
                        let mut r =
                            Vec::with_capacity(files.length().try_into().unwrap_or_default());
                        for i in 0..files.length() {
                            Extend::extend(&mut r, files.get(i));
                        }
                        r
                    })
                    .unwrap_or_default();
                drop_content.set(DropContent::Files(files));
            }
        })
    };

    html!(<>
        // Due to https://github.com/jetli/yew-hooks/issues/35 the ref currently must be on a direct element
        // of this component. It cannot be on an element nested by another component.
        <div ref={node.clone()}>
        <Bullseye>

        <Form>

            <FormGroup>
                <FileUpload
                    drag_over={*drop.over}
                >
                    <FileUploadSelect>
                        <InputGroup>
                            <TextInput readonly=true value={(*drop_content).to_string()}/>
                            <input ref={file_input_ref.clone()} style="display: none;" type="file" onchange={onchange_open} />
                            <Button
                                variant={ButtonVariant::Control}
                                disabled={processing.is_processing()}
                                onclick={onopen}
                            >
                                {"Open"}
                            </Button>
                            <Button
                                variant={ButtonVariant::Control}
                                disabled={error.is_some() || processing.is_processing()}
                                onclick={onsubmit}
                            >
                                {"Load"}
                            </Button>
                            <Button
                                variant={ButtonVariant::Control}
                                onclick={onclear}
                                disabled={drop_content.is_none()}>
                                {"Clear"}
                            </Button>
                        </InputGroup>
                    </FileUploadSelect>
                    <FileUploadDetails
                        processing={processing.is_processing()}
                        invalid={error.is_some()}
                    >
                    </FileUploadDetails>
                </FileUpload>

                {helper_text}
            </FormGroup>
        </Form>
        </Bullseye>
        </div>
        </>
    )
}
