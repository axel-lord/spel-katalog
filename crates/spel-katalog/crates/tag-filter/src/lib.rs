//! Tag filter dialog.

use ::std::collections::BTreeMap;

use ::iced_aw::{sidebar::TabLabel, widget::Sidebar};
use ::iced_core::{Alignment::Center, Border, Element, Length::Shrink};
use ::iced_runtime::Task;
use ::iced_widget as widget;
use ::itertools::Itertools;
use ::rustc_hash::FxHashSet;
use ::spel_katalog_common::{IntoOrRequest, OrRequest, in_place::PushMaybe};
use ::spel_katalog_formats::{Tag, TagFilter, TagFilterKind, TagFilterMode, TagId, TagStorage};
use ::spel_katalog_list_menu::ListMenu;
use ::spel_katalog_widget::rule;
use ::tap::Pipe;

/// A single filter layer.
#[derive(Debug, Clone)]
struct Layer {
    /// Name of layer.
    name: String,
    /// Filter mode for exclusion.
    incl_mode: TagFilterMode,
    /// Filter mode for inclusion.
    excl_mode: TagFilterMode,
    /// Tag states.
    tags: BTreeMap<Tag, TagFilterKind>,
}

impl Default for Layer {
    fn default() -> Self {
        Self::new(String::from("#0"))
    }
}

impl Layer {
    /// Construct a new layer.
    const fn new(name: String) -> Self {
        Self {
            name,
            incl_mode: TagFilterMode::All,
            excl_mode: TagFilterMode::Any,
            tags: BTreeMap::new(),
        }
    }

    /// Get two filters from internal state.
    fn unzip(&self, tag_storage: &TagStorage) -> [TagFilter<FxHashSet<TagId>>; 2] {
        let [mut incl, mut excl] = [
            TagFilter {
                kind: TagFilterKind::Include,
                mode: self.incl_mode,
                tags: FxHashSet::default(),
            },
            TagFilter {
                kind: TagFilterKind::Exclude,
                mode: self.excl_mode,
                tags: FxHashSet::default(),
            },
        ];

        for (tag, action) in &self.tags {
            match action {
                TagFilterKind::Include => &mut incl,
                TagFilterKind::Exclude => &mut excl,
            }
            .insert(tag_storage.get_id(tag));
        }

        [incl, excl]
    }
}

/// Tag filter view messages.
#[derive(Debug, Clone)]
pub enum Message {
    /// A tab was selected.
    TabSelected(usize),
    /// Add a layer.
    AddLayer,
    /// Remove selected layer.
    RemoveLayer,
    /// Sort layers.
    SortLayers,
    /// Set the value of a tag.
    SetTag {
        /// Layer to set tag in.
        layer: usize,
        /// Tag to set.
        tag: Tag,
        /// Set tag to be exluded/included/unused.
        kind: Option<TagFilterKind>,
    },

    /// Set the mode of a layer.
    SetMode {
        /// Layer to set mode in.
        layer: usize,
        /// Kind to set mode for.
        kind: TagFilterKind,
        /// Mode to set kind of layer to.
        mode: TagFilterMode,
    },
}

/// Request an action from parent.
#[derive(Debug, Clone)]
pub enum Request {
    /// Request the tag filter be set.
    SetFilter(Vec<TagFilter<FxHashSet<TagId>>>),
}

/// State of tag filter picker.
#[derive(Debug)]
pub struct TagFilterDialog {
    /// Layers of filter.
    layers: Vec<Layer>,
    /// Selected layer.
    selected: usize,
}

impl Default for TagFilterDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl TagFilterDialog {
    /// Create a new tag filter dialog.
    pub fn new() -> Self {
        TagFilterDialog {
            layers: vec![Layer::default()],
            selected: 0,
        }
    }

    /// Resolve filter from layers.
    fn to_filter(&self, tag_storage: &TagStorage) -> Vec<TagFilter<FxHashSet<TagId>>> {
        self.layers
            .iter()
            .flat_map(|layer| layer.unzip(tag_storage))
            .filter(|layer| !layer.is_empty())
            .collect()
    }

    /// Set the value of a tag in the given layer.
    pub fn set_tag(&mut self, layer: usize, tag: Tag, kind: Option<TagFilterKind>) {
        if let Some(layer) = self.layers.get_mut(layer) {
            if let Some(kind) = kind {
                layer.tags.insert(tag, kind);
            } else {
                layer.tags.remove(&tag);
            }
        }
    }

    /// Set the mode for the given kind of the given layer.
    pub fn set_mode(&mut self, layer: usize, kind: TagFilterKind, mode: TagFilterMode) {
        if let Some(layer) = self.layers.get_mut(layer) {
            match kind {
                TagFilterKind::Include => layer.incl_mode = mode,
                TagFilterKind::Exclude => layer.excl_mode = mode,
            }
        }
    }

    /// Get a new free layer name.
    fn new_layer_name(&self) -> String {
        let mut counter = 0usize;
        'outer: loop {
            let name = format!("#{counter}");
            counter += 1;
            for layer in &self.layers {
                if layer.name == name {
                    continue 'outer;
                }
            }
            return name;
        }
    }

    /// Update state.
    pub fn update(
        &mut self,
        message: Message,
        tag_storage: &TagStorage,
    ) -> Task<OrRequest<Message, Request>> {
        match message {
            Message::SetTag { layer, tag, kind } => {
                self.set_tag(layer, tag, kind);
                self.to_filter(tag_storage)
                    .pipe(Request::SetFilter)
                    .into_request()
                    .pipe(Task::done)
            }
            Message::SetMode { layer, kind, mode } => {
                self.set_mode(layer, kind, mode);
                self.to_filter(tag_storage)
                    .pipe(Request::SetFilter)
                    .into_request()
                    .pipe(Task::done)
            }
            Message::TabSelected(id) => {
                self.selected = self.layers.len().saturating_sub(1).min(id);
                Task::none()
            }
            Message::AddLayer => {
                let name = self.new_layer_name();
                self.layers
                    .insert(self.selected.max(self.layers.len()), Layer::new(name));
                Task::none()
            }
            Message::RemoveLayer => {
                if self.layers.len() > 1 {
                    self.layers.remove(self.selected);
                    self.selected = self.layers.len().saturating_sub(1).min(self.selected);
                }
                Task::none()
            }
            Message::SortLayers => {
                let current = self
                    .layers
                    .get(self.selected)
                    .map(|layer| layer.name.clone());
                self.layers.sort_by_key(|layer| {
                    layer
                        .name
                        .get(1..)
                        .and_then(|name| name.parse::<usize>().ok())
                        .unwrap_or(0)
                });
                if let Some(current) = current
                    && let Some((idx, _)) = self
                        .layers
                        .iter()
                        .enumerate()
                        .find(|(_, layer)| layer.name == current)
                {
                    self.selected = idx;
                }
                Task::none()
            }
        }
    }

    /// View dialog state.
    pub fn view(
        &self,
        tags: &TagStorage,
    ) -> Element<'_, Message, ::iced_core::Theme, widget::Renderer> {
        match self.layers.as_slice() {
            [] | [_] => self.view_layer(tags, 0).into(),
            layers @ [_, _, ..] => widget::Row::new()
                .spacing(6)
                .push(
                    layers
                        .iter()
                        .enumerate()
                        .fold(
                            Sidebar::new(Message::TabSelected).height(Shrink),
                            |sidebar, (idx, layer)| {
                                sidebar.push(idx, TabLabel::Text(layer.name.clone()))
                            },
                        )
                        .set_active_tab(&self.selected),
                )
                .push(self.view_layer(tags, self.selected))
                .height(Shrink)
                .into(),
        }
    }

    /// View dialog layer state.
    fn view_layer(&self, tags: &TagStorage, idx: usize) -> widget::Column<'_, Message> {
        widget::Column::new()
            .spacing(5)
            .align_x(Center)
            .push(::iced_aw::widget::ContextMenu::new(
                widget::text("Tag Filter").width(360).center(),
                || {
                    ListMenu::new()
                        .label("Layers")
                        .separator()
                        .button("Add New", || Message::AddLayer)
                        .button_if(self.layers.len() > 1, "Remove", || Message::RemoveLayer)
                        .button_if(self.layers.len() > 1, "Sort", || Message::SortLayers)
                        .into()
                },
            ))
            .push(rule::horizontal().pipe(widget::container).width(360))
            .push_maybe(self.layers.get(idx).map(move |layer| {
                tags.iter().chunks(5).into_iter().fold(
                    widget::Column::new()
                        .spacing(3)
                        .align_x(Center)
                        .push(
                            widget::Row::new()
                                .spacing(3)
                                .align_y(Center)
                                .push(widget::text("Inclusion Mode"))
                                .push(
                                    widget::pick_list(
                                        [TagFilterMode::Any, TagFilterMode::All],
                                        Some(layer.incl_mode),
                                        move |mode| Message::SetMode {
                                            layer: idx,
                                            kind: TagFilterKind::Include,
                                            mode,
                                        },
                                    )
                                    .padding(3),
                                )
                                .push(widget::text("Exclusion Mode"))
                                .push(
                                    widget::pick_list(
                                        [TagFilterMode::Any, TagFilterMode::All],
                                        Some(layer.excl_mode),
                                        move |mode| Message::SetMode {
                                            layer: idx,
                                            kind: TagFilterKind::Exclude,
                                            mode,
                                        },
                                    )
                                    .padding(3),
                                ),
                        )
                        .push(rule::horizontal().pipe(widget::container).width(360)),
                    |col, chunk| {
                        col.push(chunk.fold(
                            widget::Row::new().spacing(3).align_y(Center),
                            |row, key_val| {
                                let kind = layer.tags.get(key_val.key()).copied();
                                let tag = key_val.key().clone();
                                row.push(
                                    widget::button(widget::text(key_val.key().name.clone()))
                                        .style(move |theme: &::iced_core::Theme, status| {
                                            widget::button::Style {
                                                border: Border::default()
                                                    .width(1.5)
                                                    .rounded(4)
                                                    .color(match kind {
                                                        Some(TagFilterKind::Include) => {
                                                            theme
                                                                .extended_palette()
                                                                .success
                                                                .strong
                                                                .color
                                                        }
                                                        Some(TagFilterKind::Exclude) => {
                                                            theme
                                                                .extended_palette()
                                                                .danger
                                                                .strong
                                                                .color
                                                        }
                                                        None => {
                                                            theme
                                                                .extended_palette()
                                                                .background
                                                                .neutral
                                                                .color
                                                        }
                                                    }),
                                                ..widget::button::background(theme, status)
                                            }
                                        })
                                        .on_press({
                                            let tag = tag.clone();
                                            Message::SetTag {
                                                layer: idx,
                                                tag,
                                                kind: match kind {
                                                    Some(TagFilterKind::Include) => {
                                                        Some(TagFilterKind::Exclude)
                                                    }
                                                    Some(TagFilterKind::Exclude) => None,
                                                    None => Some(TagFilterKind::Include),
                                                },
                                            }
                                        })
                                        .padding(4)
                                        .pipe(widget::mouse_area)
                                        .on_right_press({
                                            let tag = tag.clone();
                                            Message::SetTag {
                                                layer: idx,
                                                tag,
                                                kind: match kind {
                                                    Some(TagFilterKind::Include) => None,
                                                    Some(TagFilterKind::Exclude) => {
                                                        Some(TagFilterKind::Include)
                                                    }
                                                    None => Some(TagFilterKind::Exclude),
                                                },
                                            }
                                        })
                                        .on_middle_press(Message::SetTag {
                                            layer: idx,
                                            tag,
                                            kind: None,
                                        }),
                                )
                            },
                        ))
                    },
                )
            }))
    }
}
