import { IconAlertTriangle } from '@tabler/icons-react'
import { toast } from '@/hooks/use-toast'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { ConfirmDialog } from '@/components/confirm-dialog'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { delete_messages } from '@/api/mailbox/envelope/api'

interface Props {
  open: boolean
  onOpenChange: (open: boolean) => void
  deleteUids: number[],
  setDeleteUids: React.Dispatch<React.SetStateAction<number[]>>;
  accountId?: number,
  mailbox?: string,
  selectedUids: number[],
}

export function EnvelopeDeleteDialog({ open, onOpenChange, deleteUids, setDeleteUids, accountId, mailbox, selectedUids }: Props) {
  const queryClient = useQueryClient();

  const deleteMutation = useMutation({
    mutationFn: ({ accountId, payload }: { accountId: number, payload: Record<string, any> }) => delete_messages(accountId, payload),
    retry: false,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['mailbox-list-messages', accountId, mailbox] });
      onOpenChange(false);
      setDeleteUids([])
      toast({
        title: 'Messages deleted successfully',
        description: 'The messages have been deleted.',
      });
    },
    onError: (error: any) => {
      toast({
        title: 'Failed to delete messages',
        description: `${error.message}`,
        variant: 'destructive',
      });
    },
  });

  const handleDelete = () => {
    if (accountId && mailbox) {
      let payload = {
        uids: deleteUids.length > 0 ? deleteUids : selectedUids,
        mailbox
      };
      deleteMutation.mutate({ accountId, payload })
    }
  }

  const isLoading = deleteMutation.isPending

  return (
    <ConfirmDialog
      open={open}
      onOpenChange={onOpenChange}
      handleConfirm={handleDelete}
      className="max-w-xl"
      isLoading={isLoading}
      title={
        <span className='text-destructive'>
          <IconAlertTriangle
            className='mr-1 inline-block stroke-destructive'
            size={18}
          />{' '}
          Move Envelope to Trash
        </span>
      }
      desc={
        <div className='space-y-4'>
          <p className='mb-2'>
            Are you sure you want to move{' '}
            <span className='font-bold'>
              {(() => {
                const emailCount = deleteUids.length > 0 ? deleteUids.length : selectedUids.length;
                return emailCount > 1 ? `this ${emailCount} emails` : 'this email';
              })()}
            </span>{' '}
            to the Trash?
            <br />
            This action will move the selected email(s) to the Trash folder. If the current mailbox is already the Trash folder, the email(s) will be permanently deleted, and cannot be recovered.
          </p>

          <Alert variant='destructive'>
            <AlertTitle>Warning!</AlertTitle>
            <AlertDescription>
              Please be cautious before proceeding.
            </AlertDescription>
          </Alert>
        </div>
      }
      confirmText='Move to Trash'
      destructive
    />
  )
}
