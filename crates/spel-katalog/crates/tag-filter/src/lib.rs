//! Tag filter dialog.

use ::std::collections::BTreeMap;

use ::iced_core::{Alignment::Center, Border, Element};
use ::iced_runtime::Task;
use ::iced_widget as widget;
use ::itertools::Itertools;
use ::rustc_hash::FxHashSet;
use ::spel_katalog_common::{IntoOrRequest, OrRequest, in_place::PushMaybe};
use ::spel_katalog_formats::{Tag, TagFilter, TagFilterKind, TagFilterMode, TagId, TagStorage};
use ::spel_katalog_widget::rule;
use ::tap::Pipe;

/// A single filter layer.
#[derive(Debug, Clone)]
struct Layer {
    /// Filter mode for exclusion.
    incl_mode: TagFilterMode,
    /// Filter mode for inclusion.
    excl_mode: TagFilterMode,
    /// Tag states.
    tags: BTreeMap<Tag, TagFilterKind>,
}

impl Default for Layer {
    fn default() -> Self {
        Self::new()
    }
}

impl Layer {
    /// Construct a new layer.
    const fn new() -> Self {
        Self {
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
            layers: vec![Layer::new()],
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
        }
    }

    /// View dialog state.
    pub fn view(
        &self,
        tags: &TagStorage,
    ) -> Element<'_, Message, ::iced_core::Theme, widget::Renderer> {
        widget::Column::new()
            .spacing(5)
            .align_x(Center)
            .push(widget::text("Tag Filter"))
            .push(rule::horizontal().pipe(widget::container).width(360))
            .push_maybe(self.layers.first().map(|layer| {
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
                                        |mode| Message::SetMode {
                                            layer: 0,
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
                                        |mode| Message::SetMode {
                                            layer: 0,
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
                                                layer: 0,
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
                                                layer: 0,
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
                                            layer: 0,
                                            tag,
                                            kind: None,
                                        }),
                                )
                            },
                        ))
                    },
                )
            }))
            .into()
    }
}
