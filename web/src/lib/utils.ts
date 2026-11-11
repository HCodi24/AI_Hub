import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
export function cn(...inputs: ClassValue[]) { return twMerge(clsx(inputs)); }
/** Last segment of a Windows or POSIX path: the project name. */
export const folderName = (path: string) => path.split(/[\\/]/).filter(Boolean).at(-1) || path;
