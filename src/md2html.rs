use pulldown_cmark::{html, Event, Options, Parser};
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
            markdown = reg.replace_all(&markdown, |caps: &Captures| {
                let label = format!("{:>016x}", rng.gen::<u64>());
                let processed = replacer(caps);
                shelter.push(Refugee {
                    label: label.clone(),
                    processed,
                });
                label
            }).to_string();
        }

        // parse markdown (= body)
        let parser =
            Parser::new_ext(&markdown, self.pulldown_cmark_options).map(|event| match event {
                Event::Html(html) => Event::Text(html),
                _ => event,
            });

        let mut body = String::new();
        html::push_html(&mut body, parser);
        for refugee in shelter {
            let Refugee { label, processed } = &refugee;
            body = body.replace(label, processed);
        }
        (self.template)(body, metadata)
    }
}
