use axum::{
    http::{HeaderMap, StatusCode},
    response::Json,
};

use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::global;

type GetDataResponse = Vec<u8>;

#[derive(Debug, Deserialize, Serialize)]
struct LEDPart {
    id: i32,
    len: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetControlDatQuery {
    dancer: String,
    #[serde(rename = "OFPARTS")]
    of_parts: HashMap<String, i32>,
    #[serde(rename = "LEDPARTS")]
    led_parts: HashMap<String, LEDPart>,
    #[serde(rename = "LEDPARTS_MERGE")]
    led_parts_merge: HashMap<String, Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetDataFailedResponse {
    err: String,
}

trait IntoResult<T, E> {
    fn into_result(self) -> Result<T, E>;
}

impl<R, E> IntoResult<R, (StatusCode, Json<GetDataFailedResponse>)> for Result<R, E>
where
    E: std::string::ToString,
{
    fn into_result(self) -> Result<R, (StatusCode, Json<GetDataFailedResponse>)> {
        match self {
            Ok(ok) => Ok(ok),
            Err(err) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(GetDataFailedResponse {
                    err: err.to_string(),
                }),
            )),
        }
    }
}

const VERSION: [u8; 2] = [0, 0];

pub async fn control_dat(
    query: Json<GetControlDatQuery>,
) -> Result<
    (StatusCode, (HeaderMap, Json<GetDataResponse>)),
    (StatusCode, Json<GetDataFailedResponse>),
> {
    let mut response: Vec<u8> = Vec::new();
    let GetControlDatQuery {
        dancer,
        of_parts,
        led_parts,
        led_parts_merge,
    } = query.0;

    for vi in VERSION {
        response.push(vi);
    }

    let of_num: u8 = of_parts.len().try_into().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(GetDataFailedResponse {
                err: "Optical Fiber number out of bounds".to_string(),
            }),
        )
    })?;

    response.push(of_num);

    let strip_num: u8 = led_parts.len().try_into().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(GetDataFailedResponse {
                err: "LED strip number out of bounds".to_string(),
            }),
        )
    })?;

    response.push(strip_num);

    let clients = global::clients::get();
    let mysql_pool = clients.mysql_pool();

    let mut parts_filter = HashSet::new();
    led_parts
        .keys()
        .for_each(|part_name| match led_parts_merge.get(part_name) {
            Some(merged_parts) => {
                merged_parts.iter().for_each(|part| {
                    parts_filter.insert(part);
                });
            }
            None => {
                parts_filter.insert(part_name);
            }
        });

    let _dancer_data = sqlx::query!(
        r#"
            SELECT
                Part.name as "part_name",
                ControlData.id as "control_data_id",
                ControlFrame.start
            FROM Dancer
            INNER JOIN Model
                ON Dancer.model_id = Model.id
            INNER JOIN Part
                ON Model.id = Part.model_id
            INNER JOIN ControlData
                ON ControlData.dancer_id = Dancer.id AND
                ControlData.part_id = Part.id
            INNER JOIN ControlFrame
                ON ControlData.frame_id = ControlFrame.id
            WHERE Dancer.name = ?
            ORDER BY ControlFrame.start
        "#,
        dancer
    )
    .fetch_all(mysql_pool)
    .await
    .into_result()?
    .into_iter()
    .filter(|data| parts_filter.contains(&data.part_name))
    .collect_vec();

    todo!()
}
