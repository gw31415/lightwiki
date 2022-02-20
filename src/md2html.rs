use base64::{encode_config, URL_SAFE_NO_PAD};
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

/// 別方法で変換する要素
struct Refugee {
    /// 変換後のテキスト
    processed: String,
    /// 変換前のテキスト
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
        let (start_label, end_label) = { // バイパスする文字列を避けた後に置換するラベル
            let mut rng = rand::thread_rng();
            (
                encode_config(&rng.gen::<u32>().to_be_bytes(), URL_SAFE_NO_PAD),
                encode_config(&rng.gen::<u32>().to_be_bytes(), URL_SAFE_NO_PAD),
            )
        };
        let mut shelter: Vec<Refugee> = Default::default();
        let label_regex = Regex::new(&format!(r"{}(\d+){}", start_label, end_label)).unwrap(); // start_labelとend_labelの間に対応するRefugeeのshelter内のインデックスが記載されている
        for rule in &self.bypass_rules {
            let (reg, replacer) = rule;
            markdown = reg
                .replace_all(&markdown, |caps: &Captures| {
                    let processed = replacer(caps);
                    shelter.push(Refugee {
                        processed,
                        source: caps[0].to_string(),
                    });
                    format!("{}{}{}", start_label, shelter.len() - 1, end_label)
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
                Event::Text(text) => {
                    if let Some(Tag::CodeBlock(_)) = tag_stack.last() {
                        let processed = label_regex
                            .replace_all(&text, |caps: &Captures| {
                                let i: usize = caps[1].parse().unwrap();
                                shelter[i].source.clone()
                            })
                            .to_string();
                        Event::Text(processed.into())
                    } else {
                        use pulldown_cmark::escape::escape_html;
                        let mut text_html_escaped = String::new();
                        escape_html(&mut text_html_escaped, &text).unwrap();
                        let processed = label_regex
                            .replace_all(&text_html_escaped, |caps: &Captures| {
                                let i: usize = caps[1].parse().unwrap();
                                shelter[i].processed.clone()
                            })
                            .to_string();
                        Event::Html(processed.into())
                    }
                }
                Event::Code(code) => {
                    let processed = label_regex
                        .replace_all(&code, |caps: &Captures| {
                            let i: usize = caps[1].parse().unwrap();
                            shelter[i].source.clone()
                        })
                        .to_string();
                    Event::Code(processed.into())
                }
                _ => event,
            });

        let mut body = String::new();
        html::push_html(&mut body, parser);
        (self.template)(body, metadata)
    }
}
