//! Noton's headless MCP host. It builds the production widget tree, not a demo.
use crate::{app::AppRoot, session_vm::SessionVm};
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use teksilo::{
    automation::{self, AutomationOp, AutomationReply, RecordingWindowOps, SettleSpec},
    prelude::*,
    widgets::*,
};
pub fn build(
    vm: SessionVm,
    locale: &str,
    size: (u32, u32),
) -> anyhow::Result<teksilo::app::HeadlessApp> {
    let client = frontend::EventHubClient::new(&vm.0.service.context.event_hub);
    client.start(vm.0.service.context.shutdown_rx.clone());
    let i18n = teksilo::i18n::I18nConfig::new()
        .framework_locales(teksilo::widgets::framework_locales())
        .source_locale("en-US".parse()?)
        .supported_locales(["en-US".parse()?, "fr-FR".parse()?])
        .user_locale(Some(locale.parse()?))
        .compile_in(&[
            ("en-US", &[include_str!("../locales/en-US.ftl")]),
            ("fr-FR", &[include_str!("../locales/fr-FR.ftl")]),
        ]);
    let mut app = TeksiloAppBuilder::new()
        .theme(crate::theme::carnet(false))
        .register_fonts(crate::theme::font_registrar())
        .install_toast(teksilo::widgets::toast::ToastInstallOptions {
            archive: None,
            ..Default::default()
        })
        .i18n(i18n)
        .event_source(crate::event_source::QleanyEventSource::new(client))
        .initial_window(
            WindowConfig::new()
                .title("Noton")
                .size(size.0, size.1)
                .root(move |tree, _| {
                    let menu = crate::app::menu();
                    let header = crate::app::plain_header(&vm, menu.clone(), tree);
                    tree.add(
                        VStack::new()
                            .child(header)
                            .child(Divider::horizontal())
                            .child(Expand::new().child(AppRoot::new(vm, menu))),
                    )
                }),
        )
        .build_headless();
    app.tree
        .layout(SizeProposal::exact(size.0 as f32, size.1 as f32));
    Ok(app)
}
fn tools() -> Value {
    json!([
        {"name":"snapshot_tree","description":"Read Noton's accessibility tree.","inputSchema":{"type":"object","properties":{"max_depth":{"type":"integer"}},"additionalProperties":false}},
        {"name":"layout_tree","description":"Read widget bounds and layout.","inputSchema":{"type":"object","properties":{"max_depth":{"type":"integer"},"include_debug":{"type":"boolean"}},"additionalProperties":false}},
        {"name":"find_node","description":"Find a node by role or label.","inputSchema":{"type":"object","properties":{"role":{"type":"string"},"label":{"type":"string"}},"additionalProperties":false}},
        {"name":"invoke_action","description":"Invoke an accessible action.","inputSchema":{"type":"object","properties":{"node":{"type":"integer"},"action":{"type":"string"}},"required":["node","action"],"additionalProperties":false}},
        {"name":"execute","description":"Execute a Teksilo AutomationOp using its externally tagged Rust/JSON DTO, e.g. {\"TypeText\":{\"node\":123,\"text\":\"hello\"}}. Mutations settle automatically.","inputSchema":{"type":"object","properties":{"operation":{"type":"object"},"settle":{"type":"object"}},"required":["operation"],"additionalProperties":false}}
    ])
}
pub fn serve(vm: SessionVm, locale: &str, size: (u32, u32)) -> anyhow::Result<()> {
    let mut app = build(vm.clone(), locale, size)?;
    let mut ops = RecordingWindowOps::new();
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        let request: Value = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                writeln!(
                    stdout,
                    "{}",
                    json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":e.to_string()}})
                )?;
                stdout.flush()?;
                continue;
            }
        };
        let Some(id) = request.get("id") else {
            continue;
        };
        let result = match request["method"].as_str().unwrap_or("") {
            "initialize" => Ok(
                json!({"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"noton","version":env!("CARGO_PKG_VERSION")}}),
            ),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools":tools()})),
            "tools/call" => {
                // A stdio host has no desktop event loop. Poll the same save
                // coordinator as the frame callback before each observation.
                vm.report(vm.tick());
                let _ = automation::execute(
                    &mut app.tree,
                    &mut ops,
                    &AutomationOp::Settle,
                    &SettleSpec::default(),
                );
                let params = &request["params"];
                let name = params["name"].as_str().unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                let operation = match name {
                    "snapshot_tree" => Some(json!({"SnapshotTree":args})),
                    "layout_tree" => Some(json!({"LayoutTree":args})),
                    "find_node" => Some(json!({"FindNode":args})),
                    "invoke_action" => Some(json!({"InvokeAction":args})),
                    "execute" => Some(args["operation"].clone()),
                    _ => None,
                };
                match operation {
                    None => Err((-32602, "Unknown tool".to_owned())),
                    Some(value) => {
                        let parsed = serde_json::from_value::<AutomationOp>(value);
                        let reply = match parsed {
                            Ok(op) => {
                                let settle = if name == "execute" {
                                    args.get("settle")
                                        .cloned()
                                        .map(serde_json::from_value)
                                        .transpose()
                                } else {
                                    Ok(None)
                                };
                                match settle {
                                    Ok(settle) => {
                                        // Exercise paint as well as layout: custom widgets can
                                        // fail at render time, and text geometry is paint-backed.
                                        app.tree.render().debug_validate_stacks();
                                        let reply = automation::execute(
                                            &mut app.tree,
                                            &mut ops,
                                            &op,
                                            &settle.unwrap_or_else(SettleSpec::default),
                                        );
                                        app.tree.render().debug_validate_stacks();
                                        reply
                                    }
                                    Err(e) => AutomationReply::err("BAD_ARGUMENT", e.to_string()),
                                }
                            }
                            Err(e) => AutomationReply::err("BAD_ARGUMENT", e.to_string()),
                        };
                        Ok(
                            json!({"content":[{"type":"text","text":serde_json::to_string(&reply)?}],"isError":matches!(reply,AutomationReply::Err{..})}),
                        )
                    }
                }
            }
            _ => Err((-32601, "Method not found".to_owned())),
        };
        let response = match result {
            Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
            Err((code, message)) => {
                json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
            }
        };
        writeln!(stdout, "{response}")?;
        stdout.flush()?;
    }
    vm.close()?;
    vm.0.service.context.shutdown();
    Ok(())
}
