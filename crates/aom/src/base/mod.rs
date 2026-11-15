use crate::{Accessibility, AccessibilityRole, Element, ElementData, IntoView};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BuiltinComponent {
    Div,
    P,
    Button,
    Input,
    List,
    Li,
    Text,
}

impl BuiltinComponent {
    pub const fn tag_name(self) -> &'static str {
        match self {
            Self::Div => "div",
            Self::P => "p",
            Self::Button => "button",
            Self::Input => "input",
            Self::List => "list",
            Self::Li => "li",
            Self::Text => "text",
        }
    }
}

macro_rules! impl_builtin_element {
    ($name:ident, $variant:ident) => {
        #[derive(Clone)]
        pub struct $name {
            data: ElementData,
        }
        impl $name {
            pub fn new() -> Self {
                Self {
                    data: ElementData::new(),
                }
            }
            pub fn with_id(mut self, id: impl Into<String>) -> Self {
                self.data.element_id = Some(id.into());
                self
            }
            pub fn with_class(mut self, class: impl Into<String>) -> Self {
                self.data
                    .classes
                    .extend(class.into().split_whitespace().map(str::to_owned));
                self
            }
            pub fn with_text(mut self, text: impl Into<String>) -> Self {
                self.data.text = Some(text.into());
                self
            }
            pub fn with_accessibility(
                mut self,
                role: AccessibilityRole,
                label: impl Into<String>,
                focusable: bool,
            ) -> Self {
                self.data.accessibility = Accessibility {
                    role,
                    label: Some(label.into()),
                    focusable,
                    disabled: false,
                };
                self
            }
            pub fn with_disabled(mut self, disabled: bool) -> Self {
                self.data.accessibility.disabled = disabled;
                self
            }
            pub fn child(mut self, child: impl Element + 'static) -> Self {
                self.data.children.push(Box::new(child));
                self
            }
            pub fn children(
                mut self,
                children: impl IntoIterator<Item = Box<dyn Element>>,
            ) -> Self {
                self.data.children.extend(children);
                self
            }
        }
        impl Element for $name {
            fn data(&self) -> &ElementData {
                &self.data
            }
            fn data_mut(&mut self) -> &mut ElementData {
                &mut self.data
            }
            fn tag_name(&self) -> &'static str {
                BuiltinComponent::$variant.tag_name()
            }
        }
    };
}

impl_builtin_element!(Div, Div);
impl_builtin_element!(P, P);
impl_builtin_element!(Button, Button);
impl_builtin_element!(Input, Input);
impl_builtin_element!(List, List);
impl_builtin_element!(Li, Li);
impl_builtin_element!(Text, Text);

impl IntoView for String {
    fn into_view(self) -> Box<dyn Element> {
        Box::new(Text::new().with_text(self))
    }
}

impl IntoView for &str {
    fn into_view(self) -> Box<dyn Element> {
        Box::new(Text::new().with_text(self))
    }
}

macro_rules! impl_text_view {
    ($($type:ty),* $(,)?) => { $(
        impl IntoView for $type {
            fn into_view(self) -> Box<dyn Element> {
                Box::new(Text::new().with_text(self.to_string()))
            }
        }
    )* };
}

impl_text_view!(
    bool, char, usize, u8, u16, u32, u64, isize, i8, i16, i32, i64, f32, f64
);
