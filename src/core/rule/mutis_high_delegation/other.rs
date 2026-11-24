/// The same protocol for a worker that is not DeepSeek (for example another Codex model). It is a capable model, so a task needs the goal
/// and its limits, not the exact edit DeepSeek's does.
pub(super) const OTHER_PROTOCOL_RULE: &str = concat!(
    protocol_heading!(),
    "- You coordinate. Implementation goes to the worker, a capable coding model; you analyse, write acceptance criteria, review and verify. Do not edit project files yourself while delegating.\n",
    split_rule!(),
    delegate_block_start!(),
    "{\"title\":\"short unique task name\",\"files\":[\"path/or/folder\"],\"instructions\":\"the goal, the constraints and where to look\",\"acceptance\":[\"a condition you can check in the diff or by running a check\"]}\n",
    delegate_block_end!(),
    "- Name the files or areas the worker should touch and what must hold afterwards. It does not need line ranges or the exact change, but anything it cannot find in the project (decisions, names, values) belongs in the instructions.\n",
    plan_all_tasks!(),
    "- Do not make running tests, builds or type checks part of the worker's task. The independent checker runs them and its verdict is part of the report; when there is no checker you run them yourself after the report.\n",
    after_delegate_blocks!(),
    review_delegate_result!(),
    "- The app counts attempts per title: the worker gets 3 attempts when a checker exists (the checker checks each), 4 without one. When they are spent it tells you to implement the task yourself and verify it (the user then judges the result).\n",
    direct_work_rule!(),
    vietnamese_rule_bullet!(),
);
