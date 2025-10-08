/*
 * Copyright © 2025 rustmailer.com
 * Licensed under RustMailer License Agreement v1.0
 * Unauthorized use or distribution is prohibited.
 */

import { cn, formatFileSize } from "@/lib/utils"
import { Badge } from "@/components/ui/badge"
import { formatDistanceToNow } from "date-fns"
import { EmailEnvelope, getBadgeVariantFromFlag, gmail_unread, isCustomFlag, seen } from "../data/schema"
import { MailIcon, MailOpen, Paperclip, Trash2 } from "lucide-react"
import { Skeleton } from "@/components/ui/skeleton"
import { Checkbox } from "@/components/ui/checkbox"
import { MailboxDialogType } from "../context"

interface MailListProps {
    items: EmailEnvelope[],
    isLoading: boolean
    currentEnvelope: EmailEnvelope | undefined
    onEnvelopeChanged: (envelope: EmailEnvelope) => void
    setOpen: (str: MailboxDialogType | null) => void,
    setSelectedIds: React.Dispatch<React.SetStateAction<string[]>>;
    setDeleteIds: React.Dispatch<React.SetStateAction<string[]>>;
    selectedIds: string[];
}

export function MailList({
    setOpen,
    items,
    currentEnvelope,
    setDeleteIds,
    isLoading,
    onEnvelopeChanged,
    setSelectedIds,
    selectedIds
}: MailListProps) {
    const handleDelete = (envelope: EmailEnvelope) => {
        setDeleteIds([envelope.id]);
        setOpen("move-to-trash");
    }

    const isGmailApi = items.some(item => item.id && isNaN(Number(item.id)));

    const handleCheckboxChange = (value: boolean | 'indeterminate', id: string) => {
        if (value === true) {
            setSelectedIds((prev) => [...prev, id]);
        } else if (value === false) {
            setSelectedIds((prev) => prev.filter((x) => x !== id));
        }
    };

    if (isLoading) {
        return (
            <div className="flex flex-col gap-2 p-2">
                {Array.from({ length: 8 }).map((_, index) => (
                    <div key={index} className="flex flex-col gap-2 p-2 rounded-lg border">
                        <div className="flex items-center gap-2">
                            <Skeleton className="h-4 w-4 rounded" />
                            <Skeleton className="h-3 w-24 rounded" />
                            <Skeleton className="h-2 w-2 rounded-full ml-auto" />
                        </div>
                        <div className="flex flex-col gap-1 pl-6">
                            <Skeleton className="h-3 w-3/4 rounded" />
                        </div>
                    </div>
                ))}
            </div>
        )
    }

    return (
        <div className="grid grid-cols-1 gap-1.5 p-1 sm:p-2">
            {items.map((item) => {
                const isUnread = item.labels && item.labels.length > 0
                    ? gmail_unread(item)
                    : !seen(item);
                const hasAttachments = item.attachments && item.attachments.length > 0;
                const attachmentCount = item.attachments?.length || 0;

                return (
                    <div
                        key={item.id}
                        className={cn(
                            "flex flex-col gap-1.5 p-2 rounded-lg border transition-all cursor-pointer",
                            "hover:bg-accent/50",
                            currentEnvelope?.id === item.id && "bg-accent",
                            selectedIds.includes(item.id) && "bg-primary/5"
                        )}
                        onClick={() => onEnvelopeChanged(item)}
                    >
                        <div className="flex items-center gap-1.5">
                            <Checkbox
                                checked={
                                    selectedIds.includes(item.id)
                                }
                                onCheckedChange={(checked) => {
                                    handleCheckboxChange(checked, item.id)
                                }}
                                onClick={(e) => e.stopPropagation()}
                                className="h-4 w-3 shrink-0"
                            />

                            <div className="flex items-center gap-1 min-w-0 flex-1">
                                {isUnread ? (
                                    <MailIcon className="h-3.5 w-3.5 text-blue-600 shrink-0" />
                                ) : (
                                    <MailOpen className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
                                )}
                                <span className="text-xs text-muted-foreground">
                                    {isGmailApi ? `mid: ${item.id}` : `uid: ${item.id}`}
                                </span>

                                <p className={cn(
                                    "text-xs font-medium truncate ml-1",
                                    isUnread && "font-semibold"
                                )}>
                                    {item.from ? `${item.from.name || ""} <${item.from.address}>` : "Unknown"}
                                </p>

                                {isUnread && (
                                    <span className="flex h-1.5 w-1.5 rounded-full bg-blue-600 shrink-0 ml-1" />
                                )}
                            </div>

                            <div className="flex items-center gap-2 shrink-0">
                                {hasAttachments && (
                                    <div className="flex items-center gap-0.5 mr-1">
                                        <Paperclip className={cn(
                                            "h-3 w-3",
                                            isUnread ? "text-blue-600" : "text-muted-foreground"
                                        )} />
                                        <span className={cn(
                                            "text-xs",
                                            isUnread ? "text-blue-600 font-medium" : "text-muted-foreground"
                                        )}>
                                            {attachmentCount}
                                        </span>
                                    </div>
                                )}

                                <span className="text-xs text-muted-foreground">
                                    {formatFileSize(item.size)}
                                </span>

                                <span className={cn(
                                    "text-xs",
                                    currentEnvelope?.id === item.id
                                        ? "text-foreground font-medium"
                                        : "text-muted-foreground"
                                )}>
                                    {item.date && formatDistanceToNow(new Date(item.date), {
                                        addSuffix: true,
                                    })}
                                </span>
                            </div>
                        </div>

                        <div className="flex items-center pl-7">
                            <h3 className={cn(
                                "text-xs line-clamp-1",
                                isUnread ? "font-medium" : "text-muted-foreground"
                            )}>
                                {item.subject || "(No Subject)"}
                            </h3>
                        </div>

                        <div className="flex items-center justify-between pl-7">
                            <div className="flex flex-wrap gap-1">
                                {item.flags?.map((flag) => (
                                    <Badge
                                        key={flag.flag + (flag.custom || "")}
                                        variant={getBadgeVariantFromFlag(flag.flag)}
                                        className="h-4 px-1 text-[11px] leading-none"
                                    >
                                        {isCustomFlag(flag.flag) ? flag.custom : flag.flag}
                                    </Badge>
                                ))}
                                {item.labels?.length > 0 &&
                                    item.labels.map((label) => (
                                        <Badge
                                            key={label}
                                            variant="outline"
                                            className="h-4 px-1 text-[11px] leading-none"
                                        >
                                            {label}
                                        </Badge>
                                    ))}
                            </div>
                            <button
                                className="text-muted-foreground hover:text-destructive transition-colors p-0.5"
                                onClick={(e) => {
                                    e.stopPropagation();
                                    handleDelete(item);
                                }}
                            >
                                <Trash2 className="h-3.5 w-3.5" />
                            </button>
                        </div>
                    </div>
                );
            })}
        </div>
    );
}