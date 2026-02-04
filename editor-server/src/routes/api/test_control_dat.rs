use std::collections::{HashMap, HashSet};

use axum::{
    http::{HeaderMap, HeaderValue, StatusCode},
    response::Json,
};

use super::types::GetDataFailedResponse;
use super::utils::{write_little_endian, IntoResult};
use itertools::Itertools;

use crate::{
    global::{self, channel_table::ChannelTable},
    routes::api::types::LEDPart,
};

type GetDataResponse = Vec<u8>;

const VERSION: [u8; 2] = [0, 0];
const TOTAL_OF_NUM: i32 = 40;
const TOTAL_STRIP_NUM: i32 = 8;

pub async fn test_control_dat() -> Result<
    (StatusCode, (HeaderMap, Json<GetDataResponse>)),
    (StatusCode, Json<GetDataFailedResponse>),
> {
    let mut response: Vec<u8> = Vec::new();
    for v in VERSION {
        response.push(v);
    }

    // let GetControlDatQuery {
    //     dancer,
    //     of_parts,
    //     led_parts,
    // } = query.0;

    let dancer = "2_feng".to_string();
    let of_parts: HashMap<String, i32> = HashMap::new();
    let mut led_parts = HashMap::new();
    led_parts.insert("mask_LED".to_string(), LEDPart { id: 40, len: 28 });

    let mut of_parts = Vec::from_iter(of_parts.into_iter());
    let mut led_parts = Vec::from_iter(led_parts.into_iter());

    ChannelTable::init();

    // TODO: find cleaner way for this
    of_parts.sort_unstable_by_key(|part| ChannelTable::get_part_id(&part.0).unwrap_or(-1));
    led_parts.sort_unstable_by_key(|part| ChannelTable::get_part_id(&part.0).unwrap_or(-1));

    let of_parts_filter: HashSet<String> =
        HashSet::from_iter(of_parts.iter().map(|(name, _)| name.clone()));

    for i in 0..TOTAL_OF_NUM {
        if let Some(name) = ChannelTable::get_part_name(i) {
            if of_parts_filter.contains(&name) {
                response.push(1);
                continue;
            }
        }

        response.push(0);
    }

    let led_parts_filter: HashSet<String> =
        HashSet::from_iter(led_parts.iter().map(|(name, _)| name.clone()));

    // let of_num: u8 = of_parts.len().try_into().map_err(|_| {
    //     (
    //         StatusCode::INTERNAL_SERVER_ERROR,
    //         Json(GetDataFailedResponse {
    //             err: "Optical Fiber number out of bounds".to_string(),
    //         }),
    //     )
    // })?;
    //
    // response.push(of_num);

    for i in 0..TOTAL_STRIP_NUM {
        if let Some(name) = ChannelTable::get_part_name(i + TOTAL_OF_NUM) {
            if led_parts_filter.contains(&name) {
                response.push(1);
                continue;
            }
        }
        response.push(0);
    }

    println!("response size now: {}", response.len());

    let mut frame_filter: HashSet<i32> = HashSet::new();

    // let strip_num: u8 = led_parts.len().try_into().map_err(|_| {
    //     (
    //         StatusCode::INTERNAL_SERVER_ERROR,
    //         Json(GetDataFailedResponse {
    //             err: "LED strip number out of bounds".to_string(),
    //         }),
    //     )
    // })?;
    //
    // response.push(strip_num);

    let clients = global::clients::get();
    let mysql_pool = clients.mysql_pool();

    // TODO: insert in order specified by the firmware team
    for (_, part) in led_parts {
        response.push(part.get_len() as u8);
    }

    let frame_data = sqlx::query!(
        r#"
            SELECT
                ControlFrame.start as "control_frame_start"
            FROM Dancer
            INNER JOIN Model
                ON Dancer.model_id = Model.id
            INNER JOIN Part
                ON Model.id = Part.model_id
            INNER JOIN ControlData
                ON Part.id = ControlData.part_id AND
                ControlData.dancer_id = Dancer.id
            INNER JOIN ControlFrame
                ON ControlData.frame_id = ControlFrame.id
            WHERE Dancer.name = ?
            ORDER BY ControlFrame.start ASC
        "#,
        dancer
    )
    .fetch_all(mysql_pool)
    .await
    .into_result()?
    .into_iter()
    .collect_vec();

    for data in &frame_data {
        frame_filter.insert(data.control_frame_start);
    }

    let frame_num: u32 = frame_filter.len() as u32;

    // response.push(frame_filter.len().try_into().map_err(|_| {
    //     (
    //         StatusCode::INTERNAL_SERVER_ERROR,
    //         Json(GetDataFailedResponse {
    //             err: "number out of bounds".to_string(),
    //         }),
    //     )
    // })?);

    write_little_endian(&frame_num, &mut response);

    let mut frames = Vec::from_iter(frame_filter.into_iter());
    frames.sort();

    frames.iter().for_each(|f| {
        println!("frame start time: {}", f);
        write_little_endian(&(*f as u32), &mut response);
    });

    let mut headers = HeaderMap::new();
    headers.insert("content-type", HeaderValue::from_static("application/json"));

    Ok((StatusCode::OK, (headers, Json(response))))
}
