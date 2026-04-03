use std::collections::HashMap;
use std::sync::Arc;

use mytex::config::load_command_config;
use mytex::errors::ParseErrorKind;
use mytex::lsp_tree_checker::check_tree;
use mytex::models::config::{CommandConfig, EnvConfig, TemplateConfig, WrapConfig};
use mytex::models::node::Node;
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
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: SemanticTokensLegend {
                                token_types: vec![
                                    SemanticTokenType::KEYWORD,
                                    SemanticTokenType::STRING,
                                ],
                                token_modifiers: vec![],
                            },
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            ..Default::default()
                        },
                    ),
                ),
                document_on_type_formatting_provider: Some(DocumentOnTypeFormattingOptions {
                    first_trigger_character: "\n".to_string(),
                    more_trigger_character: None,
                }),
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
        // completions.extend(
        //     self.parser_command_config
        //         .values()
        //         .map(|value| CompletionItem {
        //             label: key.clone(),
        //             kind: Some(CompletionItemKind::FUNCTION),
        //             insert_text: Some(key.clone()),
        //             ..Default::default()
        //         })
        //         .collect::<Vec<_>>(),
        // );
        //
        //スニペット追加
        for config in self.parser_command_config.values() {
            match config {
                CommandConfig::Template(TemplateConfig {
                    args_count,
                    completion_label: Some(label), //labelの存在もこのブロックの条件
                    completion_template,
                    ..
                }) => {
                    let insert_text = match completion_template {
                        Some(template) => template.to_string(),
                        None => {
                            let snippet = (1..*args_count)
                                .map(|i| format!("\n    ${}", i))
                                .collect::<String>();
                            format!("{}{}", label, snippet)
                        }
                    };
                    completions.push(create_completion(label, "command", &insert_text));
                }
                CommandConfig::Wrap(WrapConfig {
                    completion_label: Some(label),
                    completion_template,
                    ..
                }) => {
                    let insert_text = match completion_template {
                        Some(template) => template.to_string(),
                        None => {
                            format!("{}\n    $1", label)
                        }
                    };

                    completions.push(create_completion(label, "wrapper", &insert_text));
                }
                CommandConfig::Env(EnvConfig {
                    completion_label: Some(label),
                    completion_template: Some(template),
                    ..
                }) => {
                    completions.push(create_completion(label, "env_command", template));
                }
                _ => (),
            }
        }
        // useful snips
        // completions.push(CompletionItem {
        //     label: "frac".to_string(),
        //     kind: Some(CompletionItemKind::SNIPPET),
        //     insert_text: Some("frac\n    $1\n    $2".to_string()),
        //     insert_text_format: Some(InsertTextFormat::SNIPPET),
        //     ..Default::default()
        // });
        completions.push(CompletionItem {
            label: "matrix2".to_string(),
            kind: Some(CompletionItemKind::SNIPPET),
            insert_text: Some("mat\n    $1 $2\n    $3 $4".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });
        completions.push(CompletionItem {
            label: "matrix3".to_string(),
            kind: Some(CompletionItemKind::SNIPPET),
            insert_text: Some("mat\n    $1 $2 $3\n    $4 $5 $6\n    $7 $8 $9".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });
        Ok(Some(CompletionResponse::Array(completions)))
    }

    async fn semantic_tokens_full(
        &self,
        p: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let documents = self.open_documents.lock().await;
        let document = documents
            .get(&p.text_document.uri)
            .expect("ドキュメントの一時データへのアクセスに失敗しました");
        let parse_res = parse_to_tree(document, &self.parser_command_config, self.indent_unit);
        if parse_res.is_err() {
            return Ok(None);
        }
        let root = parse_res.unwrap();
        fn extends_tree(current: Node) -> Vec<SemanticToken> {
            match current {
                Node::Root {
                    children,
                    line_num,
                    leading_chars,
                } => {
                    let mut result = Vec::new();
                    for child in children {
                        result.extend(extends_tree(child));
                    }
                    result
                }
                Node::Command {
                    name,
                    config_key,
                    captures,
                    children,
                    line_num,
                    leading_chars,
                } => {
                    let mut result = Vec::new();
                    result.push(SemanticToken {
                        delta_line: line_num as u32,
                        delta_start: leading_chars as u32,
                        length: name.len() as u32,
                        token_type: 0, // SemanticTokenType::KEYWORD
                        token_modifiers_bitset: 0,
                    });

                    for child in children {
                        result.extend(extends_tree(child));
                    }
                    result
                }
                Node::Leaf {
                    content,
                    line_num,
                    leading_chars,
                } => {
                    // \command にのみ token_type:1 を付与、それ以外は無色
                    let mut tokens = Vec::new();
                    let bytes = content.as_bytes();
                    let mut i = 0;
                    while i < bytes.len() {
                        if bytes[i] == b'\\' {
                            let start = i;
                            i += 1;
                            let cmd_start = i;
                            while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
                                i += 1;
                            }
                            if i > cmd_start {
                                tokens.push(SemanticToken {
                                    delta_line: line_num as u32,
                                    delta_start: (leading_chars as usize + start) as u32,
                                    length: (i - start) as u32,
                                    token_type: 1,
                                    token_modifiers_bitset: 0,
                                });
                            }
                        } else {
                            i += 1;
                        }
                    }
                    tokens
                }
            }
        }
        // 次にdelta_line, delta_startをちゃんと前後の差分に変える
        let mut genuine_tokens: Vec<SemanticToken> = Vec::new();
        let tokens_linear = extends_tree(root);
        let mut prev_line = 0;
        let mut prev_start = 0;
        for token in tokens_linear {
            let delta_line = token.delta_line - prev_line;
            //改行なければ差分、新しい行なら絶対値を採用
            let delta_start = if delta_line == 0 {
                token.delta_start - prev_start
            } else {
                token.delta_start
            };
            genuine_tokens.push(SemanticToken {
                delta_line,
                delta_start,
                length: token.length,
                token_type: token.token_type,
                token_modifiers_bitset: token.token_modifiers_bitset,
            });
            prev_line = token.delta_line;
            prev_start = token.delta_start;
        }
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: genuine_tokens,
        })))
    }

    async fn on_type_formatting(
        &self,
        p: DocumentOnTypeFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        let Position { line, character } = p.text_document_position.position;
        self.client
            .log_message(
                MessageType::INFO,
                format!(
                    "on_type_formatting: line={} character={} ch={:?}",
                    line, character, p.ch
                ),
            )
            .await;
        let documents = self.open_documents.lock().await;
        let document = documents
            .get(&p.text_document_position.text_document.uri)
            .expect("ドキュメントの一時データへのアクセスに失敗しました");
        self.client
            .log_message(
                MessageType::INFO,
                format!(
                    "document lines: {:?}",
                    document.lines().enumerate().collect::<Vec<_>>()
                ),
            )
            .await;
        let parse_res = parse_to_tree(document, &self.parser_command_config, self.indent_unit);
        match parse_res {
            Ok(root) => {
                let tree_check_result = check_tree(
                    root,
                    self.indent_unit,
                    &self.parser_command_config,
                    Vec::new(),
                );
                self.client
                    .log_message(
                        MessageType::INFO,
                        format!("check_tree result: {:?}", tree_check_result),
                    )
                    .await;
                match tree_check_result {
                    // 成功=引数十分
                    Ok(()) => Ok(Some(vec![TextEdit {
                        range: Range {
                            start: Position { line, character: 0 },
                            end: Position {
                                line,
                                character: self.indent_unit.unwrap_or(4) as u32,
                            },
                        },
                        new_text: "".to_string(),
                    }])),
                    Err(_) => Ok(None),
                }
            }
            Err(e) => {
                self.client
                    .log_message(MessageType::INFO, format!("parse_to_tree error: {:?}", e))
                    .await;
                Ok(None)
            }
        }
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
                let line_res = check_tree(
                    root,
                    self.indent_unit,
                    &self.parser_command_config,
                    Vec::new(),
                );
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

fn create_completion(label: &str, display_kind: &str, snip_insert_text: &str) -> CompletionItem {
    let display_kind_enclosed = format!("({})", display_kind);
    CompletionItem {
        label: label.to_string(),
        detail: Some(display_kind_enclosed.clone()),
        label_details: Some(CompletionItemLabelDetails {
            detail: None,
            description: Some(display_kind_enclosed),
        }),
        kind: Some(CompletionItemKind::SNIPPET),
        insert_text: Some(snip_insert_text.to_string()),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        ..Default::default()
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
