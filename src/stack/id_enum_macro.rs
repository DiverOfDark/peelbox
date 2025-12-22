#[macro_export]
macro_rules! define_id_enum {
    (
        $(#[$enum_meta:meta])*
        $enum_name:ident {
            $(
                $(#[$variant_meta:meta])*
                $variant:ident => $serde_name:literal : $display_name:literal
                $( | $alias:literal )*
            ),* $(,)?
        }
    ) => {
        $(#[$enum_meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub enum $enum_name {
            $(
                $(#[$variant_meta])*
                $variant,
            )*
            Custom(String),
        }

        impl serde::Serialize for $enum_name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                let s = match self {
                    $(
                        Self::$variant => $serde_name,
                    )*
                    Self::Custom(name) => name,
                };
                serializer.serialize_str(s)
            }
        }

        impl<'de> serde::Deserialize<'de> for $enum_name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let s = String::deserialize(deserializer)?;
                Ok(match s.as_str() {
                    $(
                        $serde_name => Self::$variant,
                    )*
                    _ => Self::Custom(s),
                })
            }
        }

        impl $enum_name {
            pub fn name(&self) -> String {
                match self {
                    $(
                        Self::$variant => $display_name.to_string(),
                    )*
                    Self::Custom(name) => name.clone(),
                }
            }

            pub fn from_name(name: &str) -> Option<Self> {
                match name {
                    $(
                        $display_name $(| $alias)* => Some(Self::$variant),
                    )*
                    _ => None,
                }
            }

            pub fn all_variants() -> &'static [Self] {
                &[
                    $(
                        Self::$variant,
                    )*
                ]
            }
        }
    };
}

#[macro_export]
macro_rules! define_id_enum_with_display {
    (
        $(#[$enum_meta:meta])*
        $enum_name:ident {
            $(
                $(#[$variant_meta:meta])*
                $variant:ident => $serde_name:literal : $display_name:literal
                $( | $alias:literal )*
            ),* $(,)?
        }
    ) => {
        $crate::define_id_enum! {
            $(#[$enum_meta])*
            $enum_name {
                $(
                    $(#[$variant_meta])*
                    $variant => $serde_name : $display_name
                    $( | $alias )*
                ),*
            }
        }

        impl std::fmt::Display for $enum_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.name())
            }
        }
    };
}
