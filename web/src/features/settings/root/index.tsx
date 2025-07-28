/*
 * Copyright © 2025 rustmailer.com
 * Licensed under RustMailer License Agreement v1.0
 * Unauthorized use or distribution is prohibited.
 */

import ContentSection from '../components/content-section'
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { IconCheck, IconCopy } from '@tabler/icons-react'
import { useCallback, useState } from 'react'
import { ConfirmDialog } from '@/components/confirm-dialog'
import { useMutation } from '@tanstack/react-query'
import { reset_root_token } from '@/api/access-tokens/api'
import { toast } from '@/hooks/use-toast'
import { ToastAction } from '@/components/ui/toast'
import { PasswordInput } from '@/components/password-input'
import { setAccessToken } from '@/stores/authStore'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { BellRing } from 'lucide-react'

const useResetRootToken = () => {
  return useMutation({
    mutationFn: reset_root_token,
    retry: 0,
  });
};


export default function RootAccessToken() {
  const [open, setOpen] = useState<boolean>(false)
  const [newToken, setNewToken] = useState<string | null>(null)

  const [isCopied, setIsCopied] = useState<boolean>(false);
  const mutation = useResetRootToken();



  const onConfirm = useCallback(() => {
    mutation.mutate(undefined, {
      onSuccess: (data) => {
        setNewToken(data);
        setAccessToken(data);
        toast({
          title: "The root token has been reset",
          description: "Your login information has been updated. No need to log in again.",
          action: <ToastAction altText="Close">Close</ToastAction>,
        });
      }
    })
  }, [mutation]);



  const onCopy = useCallback(async () => {
    if (newToken) {
      try {
        await navigator.clipboard.writeText(newToken);
        setIsCopied(true);
      } catch (err) {
        toast({
          variant: "destructive",
          title: "Failed to copy text",
          description: (err as Error).message,
          action: <ToastAction altText="Try again">Try again</ToastAction>,
        })
      }
    }
  }, [newToken]);



  return (
    <ContentSection
      title='Root'
      desc='Change root accessToken, which has broad permissions, to invalidate the old token and generate a new one. This updates the authentication credentials to ensure secure access.'
      showHeader={false}
    >
      <Card className="flex flex-col">
        <CardHeader>
          <CardTitle>Reset Root Token</CardTitle>
          <CardDescription>To invalidate the old root access token and generate a new one.</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4">
          <Alert>
            <BellRing />
            <AlertTitle>Warning:</AlertTitle>
            <AlertDescription>
              <p className="text-sm text-warning-dark">
                The root token is used to access the admin interface. It can be reset via API or UI,
                which will invalidate the previous token. The token file can be found in the directory
                specified by either `rustmailer_root_dir` configuration or the `RUSTMAILER_ROOT_DIR`
                environment variable.
              </p>
            </AlertDescription>
          </Alert>
        </CardContent>
        <CardFooter>
          <Button className="ml-auto" onClick={() => setOpen(true)}>
            Reset
          </Button><ConfirmDialog
            key='root-token-reset'
            destructive
            open={open}
            onOpenChange={setOpen}
            handleConfirm={onConfirm}
            className='max-w-xl'
            title=""
            desc={
              <>
                <Alert>
                  <BellRing />
                  <AlertTitle>Important:</AlertTitle>
                  <AlertDescription>
                    <p className="text-sm text-warning-dark">
                      You are about to reset the root access token. This action cannot be undone.
                    </p>
                  </AlertDescription>
                </Alert>
                {newToken && <div className="flex w-full mt-8 mb-8 items-center space-x-2">
                  <PasswordInput value={newToken} className="w-full" />
                  <Button onClick={onCopy}>
                    {isCopied ? (
                      <IconCheck className="h-5 w-5" aria-hidden="true" />
                    ) : (
                      <IconCopy className="h-5 w-5" aria-hidden="true" />
                    )}
                  </Button>
                </div>}
              </>
            }
            confirmText='Reset'
            isLoading={mutation.isPending}
          />
        </CardFooter>
      </Card>
    </ContentSection>
  )
}
