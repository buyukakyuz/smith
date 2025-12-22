use crate::config::ModelRegistry;

#[derive(Debug, Clone)]
pub struct PickerModel {
    pub id: String,
    pub name: String,
}

pub struct ModelPickerModal {
    pub models: Vec<(String, Vec<PickerModel>)>,
    pub selected: usize,
    total_count: usize,
}

impl ModelPickerModal {
    #[must_use]
    pub fn new() -> Self {
        let registry = ModelRegistry::load();

        let models: Vec<_> = registry
            .all_models_by_provider()
            .into_iter()
            .map(|(provider, models)| {
                let picker_models: Vec<PickerModel> = models
                    .iter()
                    .map(|m| PickerModel {
                        id: m.id.clone(),
                        name: m.name.clone(),
                    })
                    .collect();
                (provider.to_string(), picker_models)
            })
            .collect();

        let total_count = models.iter().map(|(_, m)| m.len()).sum();

        Self {
            models,
            selected: 0,
            total_count,
        }
    }

    #[must_use]
    pub fn selected_model(&self) -> Option<String> {
        let mut idx = 0;

        for (_, models) in &self.models {
            for model in models {
                if idx == self.selected {
                    return Some(model.id.clone());
                }
                idx += 1;
            }
        }

        None
    }

    #[must_use]
    pub fn total_count(&self) -> usize {
        self.total_count
    }

    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn select_next(&mut self) {
        if self.selected + 1 < self.total_count {
            self.selected += 1;
        }
    }
}

impl Default for ModelPickerModal {
    fn default() -> Self {
        Self::new()
    }
}
