use std::collections::HashMap;

use mytex::config::load_command_config;
use mytex::errors::ParseErrorKind;
use mytex::lsp_tree_checker::check_tree;
use mytex::models::config::CommandConfig;
use mytex::parser::parse_to_tree;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

#[derive(Debug)]
struct Backend {
    client: Client,
    parser_command_config: HashMap<String, CommandConfig>,
    indent_unit: Option<usize>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..Default::default()
            },
            //のちにenv!("CARGO_PKG_VERSION")
            server_info: Some(ServerInfo {
                name: "dtex-lsp".to_string(),
                version: Some("1.0.0".to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "server initialized!")
            .await;
    }

    async fn did_open(&self, p: DidOpenTextDocumentParams) {
        let parse_res = parse_to_tree(&p.text_document.text, &self.parser_command_config, None);
        //デバッグ
        self.client
            .log_message(MessageType::INFO, format!("parse_res: {:?}", parse_res))
            .await;
        match parse_res {
            Ok(root) => {
                let line_res = check_tree(root, self.indent_unit, &self.parser_command_config);
                match line_res {
                    Ok(()) => {
                        self.client
                            .log_message(MessageType::INFO, "Completed!")
                            .await;
                    }
                    Err(e) => {
                        let diagnostic = Diagnostic::new(
                            Range {
                                start: Position {
                                    line: e.line as u32,
                                    character: e.character as u32,
                                },
                                end: Position {
                                    line: e.line as u32,
                                    character: e.character as u32,
                                },
                            },
                            Some(DiagnosticSeverity::ERROR),
                            None,
                            Some("dtex".to_string()),
                            e.kind.to_string(),
                            None,
                            None,
                        );
                        self.client
                            .publish_diagnostics(
                                p.text_document.uri.clone(),
                                vec![diagnostic],
                                None,
                            )
                            .await;
                    }
                }
            }
            Err(e) => {
                let severity = match &e.kind {
                    ParseErrorKind::DangerousCaptureGroups { .. } => DiagnosticSeverity::WARNING,
                    _ => DiagnosticSeverity::ERROR,
                };
                let diagnostic = Diagnostic::new(
                    Range {
                        start: Position {
                            line: e.line as u32,
                            character: e.character as u32,
                        },
                        end: Position {
                            line: e.line as u32,
                            character: e.character as u32,
                        },
                    },
                    Some(severity),
                    None,
                    Some("dtex".to_string()),
                    e.kind.to_string(),
                    None,
                    None,
                );
                self.client
                    .publish_diagnostics(p.text_document.uri, vec![diagnostic], None)
                    .await;
            }
        }
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        parser_command_config: load_command_config(None).expect("404"),
        indent_unit: None,
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
