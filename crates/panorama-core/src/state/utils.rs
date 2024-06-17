use itertools::Itertools;
use serde_json::{Number, Value};
use tantivy::schema::OwnedValue;

pub fn owned_value_to_json_value(data_value: &OwnedValue) -> Value {
  match data_value {
    OwnedValue::Null => Value::Null,
    OwnedValue::Str(s) => Value::String(s.to_string()),
    OwnedValue::U64(u) => Value::Number(Number::from(*u)),
    OwnedValue::I64(i) => Value::Number(Number::from(*i)),
    OwnedValue::F64(f) => Value::Number(Number::from_f64(*f).unwrap()),
    OwnedValue::Bool(b) => Value::Bool(*b),
    OwnedValue::Array(a) => {
      Value::Array(a.into_iter().map(owned_value_to_json_value).collect_vec())
    }
    OwnedValue::Object(o) => Value::Object(
      o.into_iter()
        .map(|(k, v)| (k.to_owned(), owned_value_to_json_value(v)))
        .collect(),
    ),
    _ => {
      println!("Converting unknown {:?}", data_value);
      serde_json::to_value(data_value).unwrap()
    } // OwnedValue::Date(_) => todo!(),
      // OwnedValue::Facet(_) => todo!(),
      // OwnedValue::Bytes(_) => todo!(),
      // OwnedValue::IpAddr(_) => todo!(),
      // OwnedValue::PreTokStr(_) => todo!(),
  }
}

// pub fn data_value_to_json_value(data_value: &DataValue) -> Value {
//   match data_value {
//     DataValue::Null => Value::Null,
//     DataValue::Bool(b) => Value::Bool(*b),
//     DataValue::Num(n) => Value::Number(match n {
//       Num::Int(i) => Number::from(*i),
//       Num::Float(f) => Number::from_f64(*f).unwrap(),
//     }),
//     DataValue::Str(s) => Value::String(s.to_string()),
//     DataValue::List(v) => {
//       Value::Array(v.into_iter().map(data_value_to_json_value).collect_vec())
//     }
//     DataValue::Json(v) => v.0.clone(),
//     DataValue::Bytes(s) => {
//       Value::String(String::from_utf8_lossy(s).to_string())
//     }
//     _ => {
//       println!("Converting unknown {:?}", data_value);
//       serde_json::to_value(data_value).unwrap()
//     } // DataValue::Bytes(s) => todo!(),
//       // DataValue::Uuid(_) => todo!(),
//       // DataValue::Regex(_) => todo!(),
//       // DataValue::Set(_) => todo!(),
//       // DataValue::Vec(_) => todo!(),
//       // DataValue::Validity(_) => todo!(),
//       // DataValue::Bot => todo!(),
//   }
// }
