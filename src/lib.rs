use actix_http::Response;
use serde::{Deserialize, Serialize};
use serde::ser::{SerializeStruct};
use progresslib2;
use std::sync::Mutex;
use std::collections::HashMap;

pub const FAILED_TO_ACQUIRE_LOCK: &'static str = "Failed to acquire lock";

#[derive(Debug, Serialize, Deserialize)]
pub struct GetProgressRequest {
    pub single: Option<String>,
    pub list: Option<Vec<String>>,
}

#[derive(Debug)]
struct MyProgressError {
    value: progresslib2::ProgressError,
}

impl Serialize for MyProgressError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer
    {
        let mut state = serializer.serialize_struct("ProgressError", 3)?;
        state.serialize_field("name", &self.value.name)?;
        state.serialize_field("progress_index", &self.value.progress_index)?;
        state.serialize_field("error_string", &self.value.error_string)?;
        state.end()
    }
}

#[derive(Debug)]
struct MyStageView {
    value: progresslib2::StageView,
}

impl Serialize for MyStageView {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer
    {
        let mut state = serializer.serialize_struct("StageView", 5)?;
        state.serialize_field("name", &self.value.name)?;
        state.serialize_field("index", &self.value.index)?;
        state.serialize_field("percent", &self.value.progress_percent)?;
        let my_progress_error: Option<MyProgressError> = match &self.value.errored {
            Some(e) => Some(MyProgressError { value: e.clone() }),
            None => None,
        };
        state.serialize_field("errored", &my_progress_error)?;
        state.serialize_field("currently_processing", &self.value.currently_processing)?;
        state.end()
    }
}

pub fn get_progresses_info<S: AsRef<str>>(
    progress_keys: Vec<S>,
    progholder: &Mutex<progresslib2::ProgressHolder<String>>,
) -> Result<HashMap<String, Vec<progresslib2::StageView>>, &'static str> {
    if progress_keys.is_empty() {
        return get_all_progresses_info(progholder);
    }

    match progholder.lock() {
        Err(_) => Err(FAILED_TO_ACQUIRE_LOCK),
        Ok(mut guard) => {
            let mut hashmap = HashMap::<String, Vec<progresslib2::StageView>>::new();
            for key in progress_keys.iter() {
                match guard.progresses.get_mut(key.as_ref()) {
                    // if not found, just return a map with less
                    // entries than requested
                    None => {}
                    Some(progitem) => {
                        hashmap.insert(key.as_ref().to_string(), progitem.into());
                    }
                }
            }
            Ok(hashmap)
        }
    }
}

pub fn get_all_progresses_info(
    progholder: &Mutex<progresslib2::ProgressHolder<String>>,
) -> Result<HashMap<String, Vec<progresslib2::StageView>>, &'static str> {
    match progholder.lock() {
        Err(_) => Err(FAILED_TO_ACQUIRE_LOCK),
        Ok(mut guard) => {
            let mut hashmap = HashMap::<String, Vec<progresslib2::StageView>>::new();
            for (key, progitem) in guard.progresses.iter_mut() {
                hashmap.insert(key.clone(), progitem.into());
            }
            Ok(hashmap)
        }
    }
}

// usage:
// let thing = item.map_or_else(|| None, |o| Some(o.0));
// let progress_keys = get_all_progresses_json(thing, &progholdermutex);
pub fn get_all_progresses_json(
    item: Option<GetProgressRequest>,
    progholder: &Mutex<progresslib2::ProgressHolder<String>>,
) -> Response {
    let progress_keys = match item {
        None => vec![],
        Some(prog_request) => {
            // if its a list use that
            let mut key_list = match prog_request.list {
                None => vec![],
                Some(ref vec) => vec.clone(),
            };
            // if the list didnt exist it will now be empty
            // so we check if there was a request for a single
            // then we just have a list of that single key
            if let Some(ref single) = prog_request.single {
                key_list.push(single.clone());
            }

            // technically its possible that the user requests both
            // a list and a single, in which case this key_list is
            // the combination of the two.
            key_list
        }
    };

    // now use the progress_keys to get data from the download manager:
    match get_progresses_info(progress_keys, progholder) {
        Ok(response_map) => {
            let mut stageview_map: HashMap<String, Vec<MyStageView>> = HashMap::new();
            for (key, value) in response_map {
                let mut new_vec = vec![];
                for stage_view in value {
                    new_vec.push(MyStageView { value: stage_view });
                }
                stageview_map.insert(key, new_vec);
            }

            Response::Ok().body(
                serde_json::to_string(&stageview_map).unwrap()
            ).into()
        },
        Err(e) => Response::InternalServerError().body(
            format!("Failed to get progresses: {}", e)
        ),
    }
}
