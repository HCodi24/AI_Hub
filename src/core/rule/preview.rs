//! Rule: after editing a user-visible screen, point the app's preview panel at it with one `preview:` line.
pub const RULE: &str = concat!(
    "Preview panel:\n",
    "- After you change something the user can see (a page, a dialog, a tab), end your reply with one line `preview: <url>` ",
    "naming the address that shows exactly that screen, for example `preview: localhost:3000/dashboard`.\n",
    "- For a screen of the MutiS app itself use `preview: localhost:7880/app/preview/?c=settings&tab=extensions` (the live Next dev server, ",
    "so it follows your edits without a build; c = settings or dashboard; tab = providers, general, sound, mutis, addons, extensions, pets, dev or dashboard).\n",
    "- Skip the line when nothing visible changed or you do not know the address; never guess a port."
);

#[cfg(test)]
mod tests {
    #[test]
    fn the_rule_reaches_the_first_request() {
        assert!(crate::core::rule::first_request_rules(true, true).contains(super::RULE));
    }
}
