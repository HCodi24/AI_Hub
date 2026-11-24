use super::types::{Debate, Side, SIDES};

const STANCE_RULE: &str = concat!(
    "Kết thúc câu trả lời bằng ĐÚNG một khối JSON (trong ```json ... ```) dạng ",
    "{\"agree\": true|false, \"disagreements\": [\"...\"]}. ",
    "agree = true nếu sau khi xem xét bạn đồng ý với hướng giải quyết chung với bên kia ",
    "(có thể đã gộp ý của nhau); disagreements = những điểm bạn VẪN chưa đồng ý (để trống nếu đồng ý)."
);

/// Round 1: independent proposal.
pub fn proposal_prompt(side: Side, request: &str, memory: Option<&str>) -> String {
    let request = format!("{}\n{request}", memory.unwrap_or_default());
    format!(
        "Bạn là {me} trong một cuộc tranh luận kỹ thuật với {other} để tìm hướng giải quyết ĐÚNG nhất.\n\
         Bạn chỉ được ĐỌC code trong thư mục (không sửa file, không chạy lệnh làm thay đổi gì).\n\n\
         Vấn đề cần giải quyết:\n{request}\n\n\
         Hãy đọc phần code liên quan rồi đưa ra đề xuất của bạn, gồm:\n\
         1. Hiểu vấn đề (nguyên nhân / yêu cầu thật sự).\n\
         2. Hướng giải quyết và lý do chọn.\n\
         3. Các bước cụ thể (file, hàm cần đổi).\n\
         4. Rủi ro và cách kiểm tra.\n\
         Viết ngắn gọn, bằng tiếng Việt.",
        me = side.name(),
        other = side.other().name(),
    )
}

/// Round 2+: critique the other side's latest proposal and revise.
pub fn critique_prompt(
    side: Side,
    request: &str,
    own: &str,
    other_text: &str,
    round: u32,
) -> String {
    let own_part = if own.trim().is_empty() {
        String::new()
    } else {
        format!("Ý kiến trước của BẠN:\n<<<\n{own}\n>>>\n\n")
    };
    format!(
        "Vòng {round} của cuộc tranh luận (hai bên nói lần lượt, bên kia vừa nói xong). Bạn là {me}. Vấn đề:\n{request}\n\n\
         {own_part}\
         Ý kiến mới nhất của {other}:\n<<<\n{other_text}\n>>>\n\n\
         Hãy đáp lại thẳng thắn và công bằng (vẫn chỉ đọc code, không sửa file):\n\
         - Chỗ sai, thiếu hoặc rủi ro trong ý kiến của {other} (kiểm chứng bằng code nếu cần).\n\
         - Điểm hay của {other} mà bạn nên lấy.\n\
         - Hướng giải quyết hiện tại của bạn sau khi cân nhắc (có thể gộp ý của cả hai).\n\
         Đừng đồng ý chỉ để kết thúc nhanh: chỉ đồng ý khi bạn thật sự thấy hướng đó đúng.\n\
         {STANCE_RULE}",
        me = side.name(),
        other = side.other().name(),
    )
}

/// The judge: a second, fresh Claude session that did not take part in the debate.
pub fn judge_prompt(debate: &Debate, memory: Option<&str>) -> String {
    let mut transcript = String::new();
    for round in &debate.rounds {
        for side in SIDES {
            let turn = round.turn(side);
            transcript.push_str(&format!(
                "\n=== Vòng {} · {} ===\n{}\n",
                round.index,
                side.name(),
                if turn.error.is_some() && turn.text.is_empty() {
                    "(không trả lời được)"
                } else {
                    turn.text.as_str()
                }
            ));
            if let Some(agree) = turn.agree {
                transcript.push_str(&format!(
                    "[Lập trường: {}; còn bất đồng: {}]\n",
                    if agree {
                        "đồng ý"
                    } else {
                        "chưa đồng ý"
                    },
                    if turn.disagreements.is_empty() {
                        "không".to_owned()
                    } else {
                        turn.disagreements.join("; ")
                    }
                ));
            }
        }
    }
    format!(
        "Bạn là TRỌNG TÀI độc lập (bạn KHÔNG tham gia tranh luận). Codex và Claude đã tranh luận về vấn đề sau:\n\
         {memory}\n{request}\n\nBiên bản tranh luận:{transcript}\n\n\
         Hãy đánh giá công bằng dựa trên lập luận và bằng chứng (có thể đọc code để kiểm chứng; không sửa file). \
         Chọn hướng ĐÚNG nhất: của Codex, của Claude, hoặc gộp. Trả lời CHỈ bằng một JSON:\n\
         {{\"decision\": \"kết luận ngắn gọn: làm gì và vì sao\", \"chosen\": \"codex|claude|merged\", \
         \"agreed_points\": [\"...\"], \"disagreements\": [\"điểm còn tranh cãi và bạn phân xử ra sao\"], \
         \"risks\": [\"...\"], \"plan_steps\": [\"bước 1 ...\", \"bước 2 ...\"]}}",
        memory = memory.unwrap_or_default(),
        request = debate.request,
    )
}

/// Text handed to "Chia việc" when the user accepts the verdict.
pub fn plan_request(debate: &Debate) -> String {
    let Some(verdict) = &debate.verdict else {
        return debate.request.clone();
    };
    let mut text = format!(
        "{}\n\nKết luận sau tranh luận Codex ↔ Claude (trọng tài độc lập):\n{}\n",
        debate.request, verdict.decision
    );
    if !verdict.plan_steps.is_empty() {
        text.push_str("\nCác bước đã thống nhất:\n");
        for (index, step) in verdict.plan_steps.iter().enumerate() {
            text.push_str(&format!("{}. {}\n", index + 1, step));
        }
    }
    if !verdict.risks.is_empty() {
        text.push_str(&format!(
            "\nRủi ro cần lưu ý: {}\n",
            verdict.risks.join("; ")
        ));
    }
    text
}
