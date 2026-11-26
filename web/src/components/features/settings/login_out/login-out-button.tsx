import { LogIn, LogOut } from "lucide-react";
import { Button } from "@/components/button/button";
import { settingsText as t } from "@/text/settings";

export function LoginOutButton({ connected, checking, disabled, onClick }: {
  connected: boolean;
  checking: boolean;
  disabled: boolean;
  onClick: () => void;
}) {
  return (
    <Button size="sm" disabled={disabled} onClick={onClick}>
      {connected ? <LogOut size={16} aria-hidden="true" /> : <LogIn size={16} aria-hidden="true" />}
      {checking ? t.providers.testing : connected ? t.providers.account.signOut : t.providers.account.signIn}
    </Button>
  );
}
