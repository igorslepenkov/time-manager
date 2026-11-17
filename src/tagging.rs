//! Предсказание тега задачи встроенной ONNX-моделью.

use ort::{inputs, Session, Value};

use crate::task::CompletedTask;

/// Загружает встроенную модель тегов. Паникует только если модель в бинарнике
/// повреждена — это ошибка сборки, а не пользователя.
pub fn load_model() -> Session {
    Session::builder()
        .unwrap()
        .with_optimization_level(ort::GraphOptimizationLevel::Level3)
        .unwrap()
        .commit_from_memory(include_bytes!("../model/tag_predict_model.onnx"))
        .unwrap()
}
/// Тег задачи от ONNX-модели. Если модель не справилась — пустая строка,
/// чтобы выгрузка всё равно состоялась.
pub fn predict_task_tags(model: &Session, tasks: &[CompletedTask]) -> Vec<String> {
    tasks
        .iter()
        .map(|task| predict_task_tag(model, &task.name).unwrap_or_default())
        .collect()
}

fn predict_task_tag(model: &Session, task_name: &str) -> Result<String, String> {
    let allocator = model.allocator();
    let names = vec![task_name.to_string()];
    let data = ([names.len()], names.into_boxed_slice());

    let input_value =
        Value::from_string_array(allocator, data).map_err(|error| format!("{error:?}"))?;
    let input = inputs![input_value].map_err(|error| format!("{error:?}"))?;

    let outputs = model.run(input).map_err(|error| format!("{error:?}"))?;

    let (_shape, tags) = outputs[0]
        .try_extract_raw_string_tensor()
        .map_err(|error| format!("{error:?}"))?;

    tags.first()
        .map(|tag| tag.to_string())
        .ok_or_else(|| "Model returned no tags".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn predicts_tag_with_bundled_model() {
        let model = load_model();

        let tag = predict_task_tag(&model, "write report").unwrap();

        assert!(!tag.is_empty());
    }
}
