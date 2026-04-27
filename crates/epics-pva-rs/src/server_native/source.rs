//! [`ChannelSource`] — the trait every native PVA server is generic over.
//!
//! Replaces the spvirit `PvStore` trait. Uses our own [`crate::pvdata`]
//! types, so no `spvirit_*` types appear in the public surface.

use std::sync::Arc;
use tokio::sync::mpsc;

use crate::pvdata::{FieldDesc, PvField};

/// A backend that can answer pvAccess GET / PUT / MONITOR requests for a
/// set of named PVs.
pub trait ChannelSource: Send + Sync + 'static {
    /// Enumerate every PV name this source can serve.
    fn list_pvs(&self) -> impl std::future::Future<Output = Vec<String>> + Send;

    /// True iff `name` resolves to a known PV.
    fn has_pv(&self, name: &str) -> impl std::future::Future<Output = bool> + Send;

    /// Fetch the type descriptor for a PV (used by GET-INIT and GET_FIELD).
    fn get_introspection(
        &self,
        name: &str,
    ) -> impl std::future::Future<Output = Option<FieldDesc>> + Send;

    /// Fetch the current value of a PV.
    fn get_value(&self, name: &str) -> impl std::future::Future<Output = Option<PvField>> + Send;

    /// Apply a PUT.
    fn put_value(
        &self,
        name: &str,
        value: PvField,
    ) -> impl std::future::Future<Output = Result<(), String>> + Send;

    /// True iff PUT is allowed against this PV (for ACL gating).
    fn is_writable(&self, name: &str) -> impl std::future::Future<Output = bool> + Send;

    /// Subscribe to value-change notifications. Returns `None` if unknown.
    fn subscribe(
        &self,
        name: &str,
    ) -> impl std::future::Future<Output = Option<mpsc::Receiver<PvField>>> + Send;

    /// Dispatch an RPC. The default impl returns "RPC not supported";
    /// implementors can override to provide actual RPC behaviour.
    ///
    /// Returns the response (FieldDesc, PvField) on success.
    fn rpc(
        &self,
        name: &str,
        request_desc: FieldDesc,
        request_value: PvField,
    ) -> impl std::future::Future<Output = Result<(FieldDesc, PvField), String>> + Send {
        let _ = (name, request_desc, request_value);
        async move { Err("RPC not supported by this source".to_string()) }
    }
}

/// Type-erased handle so the server runtime can hold heterogeneous sources
/// without monomorphising every async path. Most callers pass an
/// `Arc<MySource>` directly; this is mainly for the runtime internals.
pub type DynSource = Arc<dyn ChannelSourceObj>;

/// Object-safe variant of [`ChannelSource`]. Auto-implemented via blanket
/// for any `T: ChannelSource`.
pub trait ChannelSourceObj: Send + Sync {
    fn list_pvs<'a>(
        &'a self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<String>> + Send + 'a>>;
    fn has_pv<'a>(
        &'a self,
        name: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + 'a>>;
    fn get_introspection<'a>(
        &'a self,
        name: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<FieldDesc>> + Send + 'a>>;
    fn get_value<'a>(
        &'a self,
        name: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<PvField>> + Send + 'a>>;
    fn put_value<'a>(
        &'a self,
        name: &'a str,
        value: PvField,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + 'a>>;
    fn is_writable<'a>(
        &'a self,
        name: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + 'a>>;
    fn subscribe<'a>(
        &'a self,
        name: &'a str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Option<mpsc::Receiver<PvField>>> + Send + 'a>,
    >;
    fn rpc<'a>(
        &'a self,
        name: &'a str,
        request_desc: FieldDesc,
        request_value: PvField,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<(FieldDesc, PvField), String>> + Send + 'a>,
    >;
}

impl<T: ChannelSource + 'static> ChannelSourceObj for T {
    fn list_pvs<'a>(
        &'a self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<String>> + Send + 'a>> {
        Box::pin(<Self as ChannelSource>::list_pvs(self))
    }
    fn has_pv<'a>(
        &'a self,
        name: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + 'a>> {
        Box::pin(<Self as ChannelSource>::has_pv(self, name))
    }
    fn get_introspection<'a>(
        &'a self,
        name: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<FieldDesc>> + Send + 'a>> {
        Box::pin(<Self as ChannelSource>::get_introspection(self, name))
    }
    fn get_value<'a>(
        &'a self,
        name: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<PvField>> + Send + 'a>> {
        Box::pin(<Self as ChannelSource>::get_value(self, name))
    }
    fn put_value<'a>(
        &'a self,
        name: &'a str,
        value: PvField,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + 'a>> {
        Box::pin(<Self as ChannelSource>::put_value(self, name, value))
    }
    fn is_writable<'a>(
        &'a self,
        name: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + 'a>> {
        Box::pin(<Self as ChannelSource>::is_writable(self, name))
    }
    fn subscribe<'a>(
        &'a self,
        name: &'a str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Option<mpsc::Receiver<PvField>>> + Send + 'a>,
    > {
        Box::pin(<Self as ChannelSource>::subscribe(self, name))
    }
    fn rpc<'a>(
        &'a self,
        name: &'a str,
        request_desc: FieldDesc,
        request_value: PvField,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<(FieldDesc, PvField), String>> + Send + 'a>,
    > {
        Box::pin(<Self as ChannelSource>::rpc(
            self,
            name,
            request_desc,
            request_value,
        ))
    }
}
