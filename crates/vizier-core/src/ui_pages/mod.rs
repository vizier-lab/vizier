//! Interactive pages an agent draws in a WebUI reply (`specs/063-html-generative-ui`).
//!
//! A page is a complete HTML document an agent made in one conversation, kept with every version
//! it went through. [`UiPages`] (added in a later task) is the one write path: it stores each
//! version's HTML in the blob store under `ui-pages/{agent}/{page}/v{n}.html`, its metadata through
//! `UiPageStorage`, and checks it in an open tab when there is one (`verify`).

pub mod edits;
pub mod limits;
pub mod verify;
