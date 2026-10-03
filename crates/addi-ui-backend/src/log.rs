macro_rules! log {
    ($($args:tt)*) => {
        ::tracing::info!($($args)*)
    };
}

pub(crate) use log;
