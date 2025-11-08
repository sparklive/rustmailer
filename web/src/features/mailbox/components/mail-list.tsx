import { cn, formatFileSize } from "@/lib/utils"
import { Badge } from "@/components/ui/badge"
import { formatDistanceToNow } from "date-fns"
import { EmailEnvelope, getBadgeVariantFromFlag, isCustomFlag } from "../data/schema"
import { MailIcon, MailOpen, Paperclip, Trash2 } from "lucide-react"
import { Skeleton } from "@/components/ui/skeleton"
import { Checkbox } from "@/components/ui/checkbox"
import { MailboxDialogType } from "../context"

interface MailListProps {
    items: EmailEnvelope[]
    isLoading: boolean
    currentEnvelope: EmailEnvelope | undefined
    onEnvelopeChanged: (envelope: EmailEnvelope) => void
    setOpen: (str: MailboxDialogType | null) => void
    setSelectedIds: React.Dispatch<React.SetStateAction<string[]>>
    setDeleteIds: React.Dispatch<React.SetStateAction<string[]>>
    selectedIds: string[]
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
        setDeleteIds([envelope.id])
        setOpen("move-to-trash")
    }

    const handleCheckboxChange = (value: boolean | 'indeterminate', id: string) => {
        if (value === true) {
            setSelectedIds(prev => [...prev, id])
        } else if (value === false) {
            setSelectedIds(prev => prev.filter(x => x !== id))
        }
    }

    if (isLoading) {
        return (
            <div className="space-y-1 p-1">
                {Array.from({ length: 8 }).map((_, i) => (
                    <div key={i} className="flex items-center gap-2 p-1.5 rounded border">
                        <Skeleton className="h-3 w-3 rounded" />
                        <Skeleton className="h-3 flex-1" />
                        <Skeleton className="h-2 w-2 rounded-full ml-auto" />
                    </div>
                ))}
            </div>
        )
    }

    return (
        <div className="divide-y divide-border">
            {items.map((item) => {
                const isUnread = !item.is_read
                const hasAttachments = item.attachments && item.attachments.length > 0;
                const isSelected = selectedIds.includes(item.id)
                const isActive = currentEnvelope?.id === item.id

                return (
                    <div
                        key={item.id}
                        className={cn(
                            "group flex items-center gap-2 p-1.5 pr-2 rounded-md border transition-all cursor-pointer text-xs",
                            "hover:bg-accent/70",
                            isActive && "bg-accent",
                            isSelected && "bg-primary/10 ring-1 ring-primary/20"
                        )}
                        onClick={() => onEnvelopeChanged(item)}
                    >
                        {/* Checkbox */}
                        <Checkbox
                            checked={isSelected}
                            onCheckedChange={(checked) => handleCheckboxChange(checked, item.id)}
                            onClick={(e) => e.stopPropagation()}
                            className="h-3.5 w-3.5 shrink-0"
                        />

                        {/* Unread Icon */}
                        {isUnread ? (
                            <MailIcon className="h-3 w-3 text-blue-600 shrink-0" />
                        ) : (
                            <MailOpen className="h-3 w-3 text-muted-foreground shrink-0" />
                        )}

                        {/* Sender */}
                        <span
                            className={cn(
                                "font-medium truncate max-w-[140px]",
                                isUnread && "font-semibold"
                            )}
                            title={item.from?.address}
                        >
                            {item.from?.name || item.from?.address || "Unknown"}
                        </span>

                        {/* Subject + Attachments */}
                        <div className="flex-1 flex items-center gap-1 min-w-0">
                            {hasAttachments && (
                                <Paperclip className={cn("h-2.5 w-2.5 shrink-0", isUnread ? "text-blue-600" : "text-muted-foreground")} />
                            )}
                            <span className={cn("truncate", isUnread ? "font-medium" : "text-muted-foreground")}>
                                {item.subject || "(No Subject)"}
                            </span>
                        </div>

                        {/* Flags / Labels */}
                        <div className="flex items-center gap-0.5 shrink-0">
                            {item.flags?.slice(0, 2).map((flag) => (
                                <Badge
                                    key={flag.flag + (flag.custom || "")}
                                    variant={getBadgeVariantFromFlag(flag.flag)}
                                    className="h-3.5 px-1 text-xs leading-none"
                                >
                                    {isCustomFlag(flag.flag) ? flag.custom : flag.flag}
                                </Badge>
                            ))}
                            {item.labels?.[0] && (
                                <Badge variant="outline" className="h-3.5 px-1 text-xs leading-none">
                                    {item.labels[0]}
                                </Badge>
                            )}
                            {(item.flags?.length ?? 0) > 2 || item.labels?.length > 1 ? (
                                <span className="text-xs text-muted-foreground">+{((item.flags?.length ?? 0) - 2) + (item.labels?.length - 1)}</span>
                            ) : null}
                        </div>

                        {/* Size & Time */}
                        <span className="text-xs text-muted-foreground shrink-0 ml-1">
                            {formatFileSize(item.size)}
                        </span>
                        <span className={cn(
                            "text-xs shrink-0",
                            isActive ? "text-foreground font-medium" : "text-muted-foreground"
                        )}>
                            {item.date && formatDistanceToNow(new Date(item.date), { addSuffix: true })}
                        </span>

                        {/* Delete Button */}
                        <button
                            className="opacity-0 group-hover:opacity-100 transition-opacity p-0.5 text-muted-foreground hover:text-destructive"
                            onClick={(e) => {
                                e.stopPropagation()
                                handleDelete(item)
                            }}
                        >
                            <Trash2 className="h-3 w-3" />
                        </button>
                    </div>
                )
            })}
        </div>
    )
}