use std::collections::HashMap;
use std::sync::Arc;

use mytex::config::load_command_config;
use mytex::errors::ParseErrorKind;
use mytex::lsp_tree_checker::check_tree;
use mytex::models::config::CommandConfig;
use mytex::parser::parse_to_tree;
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::{Client, LspService, Server};
use tower_lsp::{LanguageServer, lsp_types::*};

#[derive(Debug)]
struct Backend {
    client: Client,
    parser_command_config: HashMap<String, CommandConfig>,
    indent_unit: Option<usize>,
    open_documents: Arc<Mutex<HashMap<Url, String>>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions::default()),
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
        let mut documents = self.open_documents.lock().await;
        // insertでドキュメントの中身を更新
        documents.insert(p.text_document.uri.clone(), p.text_document.text.clone());
        drop(documents); //この次のdiagnoseでself.documentsにアクセスできるようロック解除(これが無ければこの関数の終わりまでロック保持されるのでdiagnoseで使えない)
        self.diagnose(p.text_document.text, p.text_document.uri)
            .await;
    }

    async fn did_change(&self, p: DidChangeTextDocumentParams) {
        // did_saveではテキストドキュメントにアクセスできないのでここでフィールドを更新しておく
        let mut documents = self.open_documents.lock().await;
        documents.insert(
            p.text_document.uri.clone(),
            p.content_changes[0].text.clone(),
        );
        drop(documents); //一応無くてもいいけど今後この次の行になんかかくかもしれんから
    }

    async fn did_save(&self, p: DidSaveTextDocumentParams) {
        let text = self.open_documents.lock().await;
        let text = text
            .get(&p.text_document.uri)
            .expect("ドキュメントの一時データへのアクセスに失敗しました。想定外のエラーです。");

        self.diagnose((*text).to_owned(), p.text_document.uri).await;
    }

    async fn completion(&self, p: CompletionParams) -> Result<Option<CompletionResponse>> {
        let caret_pos = p.text_document_position.position;
        let mut completions: Vec<CompletionItem> = Vec::new();
        // add commands list
        completions.extend(
            self.parser_command_config
                .keys()
                .map(|key| CompletionItem {
                    label: key.clone(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    insert_text: Some(key.clone()),
                    ..Default::default()
                })
                .collect::<Vec<_>>(),
        );
        // useful snips
        completions.push(CompletionItem {
            label: "frac".to_string(),
            kind: Some(CompletionItemKind::SNIPPET),
            insert_text: Some("frac\n    $1\n    $2".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });
        completions.push(CompletionItem {
            label: "matrix2".to_string(),
            kind: Some(CompletionItemKind::SNIPPET),
            insert_text: Some("mat\n    $1  $2\n    $3  $4".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });
        Ok(Some(CompletionResponse::Array(completions)))
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

impl Backend {
    async fn diagnose(&self, body: String, uri: Url) {
        let parse_res = parse_to_tree(&body, &self.parser_command_config, None);
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
                        self.clear_diagnose(uri).await;
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
                            .publish_diagnostics(uri, vec![diagnostic], None)
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
                    .publish_diagnostics(uri, vec![diagnostic], None)
                    .await;
            }
        }
    }
    async fn clear_diagnose(&self, uri: Url) {
        self.client.publish_diagnostics(uri, vec![], None).await;
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
        open_documents: Arc::new(Mutex::new(HashMap::new())),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
