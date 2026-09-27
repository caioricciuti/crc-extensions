//! Write crc extensions as plain Rust functions.
//!
//! An extension is a WebAssembly module crc loads and runs in a sandbox. This
//! crate is the whole interface to it: declare your commands with
//! [`commands!`], and each one is a function from [`Input`] to [`Output`].
//!
//! ```ignore
//! use crc_extension::{Input, Output};
//!
//! fn sort(input: Input) -> Output {
//!     let mut lines: Vec<&str> = input.text.lines().collect();
//!     lines.sort_unstable();
//!     Output::replace(lines.join("\n"))
//! }
//!
//! crc_extension::commands! {
//!     "sort" => sort,
//! }
//! ```
//!
//! Build with `cargo build --release --target wasm32-unknown-unknown` and
//! list the command in `manifest.json`. What an extension may touch is
//! decided by that manifest, not by this crate: crc links only the host
//! functions the manifest's capabilities grant.
//!
//! # The interface underneath (API 1)
//!
//! The module exports `memory`, `crc_alloc(len) -> ptr`,
//! `crc_free(ptr, len)`, and one function per command taking `(ptr, len)`
//! of a UTF-8 JSON input and returning `(ptr << 32) | len` of a UTF-8 JSON
//! output. crc frees the output with `crc_free` once it has copied it.
//! [`commands!`] writes all of this; nothing else here is needed to use it.

#![forbid(unsafe_op_in_unsafe_fn)]

mod json;

/// What a command is given.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Input {
    /// The selection, or the whole document when nothing is selected.
    pub text: String,
    /// Whether `text` is the selection.
    pub selection: bool,
    /// crc's name for the document's language: `rust`, `python`, `markdown`,
    /// `text` and so on.
    pub language: String,
    /// The command's id from the manifest.
    pub command: String,
}

/// What a command answers.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Output {
    /// Replaces the text the command was given, as one undo step.
    pub replace: Option<String>,
    /// Shown in crc's status line.
    pub message: Option<String>,
    /// A whole HTML page crc shows in the preview pane beside the editor.
    /// Needs the `preview.show` capability. crc runs no script in it and
    /// loads nothing from the network; files load only from the document's
    /// folder.
    pub html: Option<String>,
}

impl Output {
    /// Replace the given text with `text`.
    pub fn replace(text: impl Into<String>) -> Output {
        Output {
            replace: Some(text.into()),
            ..Output::default()
        }
    }

    /// Change nothing, say `text`.
    pub fn message(text: impl Into<String>) -> Output {
        Output {
            message: Some(text.into()),
            ..Output::default()
        }
    }

    /// Show `page`, a whole HTML document, in the preview pane.
    pub fn html(page: impl Into<String>) -> Output {
        Output {
            html: Some(page.into()),
            ..Output::default()
        }
    }

    /// Change nothing.
    pub fn nothing() -> Output {
        Output::default()
    }

    /// Also say `text`.
    pub fn with_message(mut self, text: impl Into<String>) -> Output {
        self.message = Some(text.into());
        self
    }
}

/// Writes a line to crc's extension log (crc > Extensions shows it). Needs
/// no capability. Does nothing outside crc.
pub fn log(text: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        #[link(wasm_import_module = "crc")]
        unsafe extern "C" {
            fn log(ptr: *const u8, len: usize);
        }
        // SAFETY: crc reads `len` bytes at `ptr` from this module's memory,
        // which `text` keeps alive for the call.
        unsafe { log(text.as_ptr(), text.len()) };
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = text;
}

/// Declares the extension's commands: `"id" => function` for each, where
/// the id matches `export` in `manifest.json`. Use it once per extension.
#[macro_export]
macro_rules! commands {
    ($($id:literal => $f:path),+ $(,)?) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn crc_alloc(len: usize) -> *mut u8 {
            $crate::__private::alloc(len)
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn crc_free(ptr: *mut u8, len: usize) {
            // SAFETY: crc hands back exactly what crc_alloc or a command
            // gave it, once.
            unsafe { $crate::__private::free(ptr, len) }
        }

        $(
            const _: () = {
                #[unsafe(export_name = $id)]
                pub unsafe extern "C" fn command(ptr: *mut u8, len: usize) -> u64 {
                    // SAFETY: crc passes a buffer it got from crc_alloc and
                    // filled, and does not touch it again.
                    unsafe { $crate::__private::run(ptr, len, $id, $f) }
                }
            };
        )+
    };
}

#[doc(hidden)]
pub mod __private {
    use super::{Input, Output};

    // Every buffer crossing the boundary is a Box<[u8]>, whose length is
    // its capacity, so crc_free(ptr, len) always frees with the right size.

    pub fn alloc(len: usize) -> *mut u8 {
        Box::into_raw(vec![0u8; len].into_boxed_slice()) as *mut u8
    }

    /// # Safety
    /// `ptr` and `len` must come from [`alloc`] or from a command's result,
    /// and be freed once.
    pub unsafe fn free(ptr: *mut u8, len: usize) {
        if !ptr.is_null() {
            // SAFETY: a Box<[u8]> of exactly `len` bytes, per the contract.
            drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len)) });
        }
    }

    /// # Safety
    /// `ptr` must come from [`alloc`] with `len`, filled by crc.
    pub unsafe fn run(ptr: *mut u8, len: usize, id: &str, f: fn(Input) -> Output) -> u64 {
        // SAFETY: the Box<[u8]> from alloc(len), filled by crc and handed over.
        let bytes = unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len)) };
        let mut input = crate::json::read_input(&bytes).unwrap_or_default();
        drop(bytes);
        input.command = id.to_owned();
        let output = f(input);
        let out = crate::json::write_output(&output)
            .into_bytes()
            .into_boxed_slice();
        let len = out.len() as u64;
        let ptr = Box::into_raw(out) as *mut u8 as u64;
        (ptr << 32) | len
    }
}
