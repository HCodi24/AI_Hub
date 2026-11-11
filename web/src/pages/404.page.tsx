import Link from "next/link";
import { chatText } from "@/text/chat";
import { routes } from "@/routes/routes";
export default function NotFound() {
  return <main><h1>{chatText.notFound}</h1><Link href={routes.home}>{chatText.home}</Link></main>;
}
