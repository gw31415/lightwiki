use pulldown_cmark::{html, Event, Options, Parser, Tag};
use rand::Rng;
use regex::{Captures, Regex};

/// メインの内容とメタデータからHTMLを出力する関数のポインタ
pub type Template = fn(String, MetaData) -> String;

/// 正規表現にマッチしたRegex::Capturesから新しい表現を出力する
pub type Replacer = fn(&Captures) -> String;

/// Markdown to Html コンバータ
pub struct Converter {
    /// MetaDataからHtmlにするテンプレート
    pub template: Template,
    /// Markdown Parser を通さないパターンとその処理方法の定義
    pub bypass_rules: Vec<(Regex, Replacer)>,
    /// pulldown_cmarkに渡すオプション
    pub pulldown_cmark_options: Options,
}

/// Wikiページの構成要素
pub struct MetaData {
    /// 項目名
    pub entry_name: String,
    /// Wikiの名前
    pub wiki_name: String,
}

struct Refugee {
    label: String,
    processed: String,
    source: String,
}

impl Converter {
    /// Converterの初期化
    pub fn new(template: Template) -> Self {
        Converter {
            template,
            bypass_rules: Default::default(),
            pulldown_cmark_options: Options::empty(),
        }
    }

    /// MarkdownをHtmlに変換します
    pub fn convert(&self, mut markdown: String, metadata: MetaData) -> String {
        // 特定の文字列をバイパス
        let mut rng = rand::thread_rng();
        let mut shelter: Vec<Refugee> = Default::default();
        for rule in &self.bypass_rules {
            let (reg, replacer) = rule;
            markdown = reg
                .replace_all(&markdown, |caps: &Captures| {
                    let label = format!("{:>016x}", rng.gen::<u64>());
                    let processed = replacer(caps);
                    shelter.push(Refugee {
                        label: label.clone(),
                        processed,
                        source: caps[0].to_string(),
                    });
                    label
                })
                .to_string();
        }

        // parse markdown (= body)
        let mut tag_stack: Vec<Tag> = Default::default();
        let parser =
            Parser::new_ext(&markdown, self.pulldown_cmark_options).map(|event| match event {
                Event::Html(html) => Event::Text(html),
                Event::Start(tag) => {
                    tag_stack.push(tag.clone());
                    Event::Start(tag)
                }
                Event::End(tag) => {
                    tag_stack.pop();
                    Event::End(tag)
                }
                Event::Text(mut text) => {
                    if let Some(Tag::CodeBlock(_)) = tag_stack.last() {
                        for refugee in &shelter {
                            let Refugee { label, source, .. } = &refugee;
                            text = text.replace(label, &source).into();
                        }
                        Event::Text(text)
                    } else {
                        for refugee in &shelter {
                            let Refugee {
                                label, processed, ..
                            } = &refugee;
                            text = text.replace(label, &processed).into();
                        }
                        Event::Html(text)
                    }
                }
                Event::Code(mut code) => {
                    for refugee in &shelter {
                        let Refugee { label, source, .. } = &refugee;
                        code = code.replace(label, &source).into();
                    }
                    Event::Code(code)
                }
                _ => event,
            });

        let mut body = String::new();
        html::push_html(&mut body, parser);
        (self.template)(body, metadata)
    }
}
