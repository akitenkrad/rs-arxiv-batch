use crate::common::{Paper, Summary};
use crate::utils::s;
use anyhow::Result;
use openai_tools::json_schema::JsonSchema;
use openai_tools::{Message, OpenAI, ResponseFormat};
use std::include_str;
use std::thread::sleep;

#[derive(Clone, Debug)]
pub struct AI {
    model_id: String,
}

impl AI {
    pub fn new(model_id: &str) -> AI {
        AI {
            model_id: String::from(model_id),
        }
    }

    async fn get_instruction(&self, paper: &Paper) -> Result<(String, String, String)> {
        // instruction
        let instruction = s(include_str!("instructions/instruction_1.txt"));

        assert!(
            paper.original_text_map.len() > 0,
            "Failed to get instruction: Original text is empty."
        );

        // paper
        let paper_text = paper.original_text2xml();
        let mut paper_xml = s("############ 論文 ############\n");
        paper_xml.push_str(&paper_text);

        // reference instruction
        let references = paper.references2xml();
        let mut reference_xml = s("############ 参考文献 ############\n");
        reference_xml.push_str(&references);

        return Ok((instruction, paper_xml, reference_xml));
    }

    async fn get_messages(&self, paper: &Paper) -> Result<Vec<Message>> {
        let (instruction, paper_xml, _) = self.get_instruction(paper).await?;
        let messages = vec![
            Message::new("system", "あなたは優秀な研究アシスタントです．"),
            Message::new(
                "user",
                &format!(
                    "これからこの論文の要約の準備をしてください: {}",
                    paper.title
                ),
            ),
            Message::new(
                "user",
                &format!("要約の際は以下の指示に従ってください: \n\n{}", instruction),
            ),
            // Message::new(
            //     "user",
            //     &format!(
            //         "以下は，論文の参考文献のリストです．要約のための参考にしてください．必要であれば論文の主張を補うために参照してください．\n\n{}",
            //         reference_xml
            //     ),
            // ),
            Message::new(
                "user",
                &format!("以下は，論文の内容です．\n\n{}", paper_xml),
            ),
            Message::new("user", "要約してください:"),
        ];
        return Ok(messages);
    }

    fn get_json_schema(&self) -> JsonSchema {
        let mut json_schema = JsonSchema::new("summary");
        json_schema.add_property(
            "is_survey",
            "boolean",
            Option::from(s("この論文がサーベイ論文かどうかをtrue/falseで判定する．")),
        );
        json_schema.add_property(
            "overview",
            "string",
            Option::from(s("この論文の概要を3文程度で記述する．")),
        );
        json_schema.add_property(
            "research_question",
            "string",
            Option::from(s("この論文のリサーチクエスチョンを説明する．この論文の背景や既存研究との関連も含めて記述する．4文程度で詳細に記述する．")),
        );
        json_schema.add_property(
            "task_category",
            "string",
            Option::from(s("この論文のタスク分類を記述する．例として，自然言語処理の場合は機械読解，機械翻訳，テキスト分類などが挙げられる．")),
        );
        json_schema.add_property(
            "domain_as_words",
            "string",
            Option::from(s(
                "この論文の実験が対象にしているドメインを単語で出力する．",
            )),
        );
        json_schema.add_property(
            "task_as_words",
            "string",
            Option::from(s("この論文のタスク分類を単語で出力する．")),
        );
        json_schema.add_property(
            "comparison_with_related_works",
            "string",
            Option::from(s("関連研究と比較した場合のこの論文の新規性について説明する．可能な限り既存研究を参照しながら記述すること．4文程度で詳細に記述する．")),
        );
        json_schema.add_property(
            "proposed_method",
            "string",
            Option::from(s(
                "この論文で使用されている手法の詳細について，一つずつ順を追って説明する．4文程度で詳細に記述する．",
            )),
        );
        json_schema.add_property(
            "datasets",
            "string",
            Option::from(s(
                "この論文で使用されているデータセットをリストアップする．",
            )),
        );
        json_schema.add_property(
            "experiments",
            "string",
            Option::from(s(
                "実験の設定と結果について詳細に説明する．4文程度で詳細に記述する．",
            )),
        );
        json_schema.add_property(
            "analysis",
            "string",
            Option::from(s(
                "実験結果の分析について記述する．4文程度で詳細に記述する．",
            )),
        );
        json_schema.add_property(
            "contributions",
            "string",
            Option::from(s("この論文のコントリビューションをリスト形式で記述する．")),
        );
        json_schema.add_property(
            "future_works",
            "string",
            Option::from(s(
                "未解決の課題および将来の研究の方向性について記述．3文程度で詳細に記述する．",
            )),
        );
        return json_schema;
    }

    pub async fn summarize(&self, paper: &mut Paper) -> Result<()> {
        let mut messages = self.get_messages(paper).await?;
        let json_schema = self.get_json_schema();

        let mut retry_count = 5u8;
        while retry_count > 0 {
            let mut openai = OpenAI::new();
            openai
                .model_id(&self.model_id)
                .messages(messages.clone())
                .temperature(1.0)
                .response_format(ResponseFormat::new("json_schema", json_schema.clone()));

            let response = match openai.chat() {
                Ok(response) => response,
                Err(e) => {
                    eprintln!("Failed to chat: {} (retry: {})", e.to_string(), retry_count);
                    retry_count -= 1;
                    sleep(std::time::Duration::from_secs(1));

                    messages.push(Message::new("system", "JSON形式で出力してください．"));
                    messages.push(Message::new("user", "要約してください．"));
                    continue;
                }
            };
            let summary = response.choices[0].message.content.clone();
            let sumamry = serde_json::from_str::<Summary>(summary.as_str())?;

            paper.summary = sumamry;

            return Ok(());
        }
        return Err(anyhow::anyhow!("Failed to summarize."));
    }
}
