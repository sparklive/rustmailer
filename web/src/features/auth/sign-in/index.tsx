import Logo from '@/assets/logo.svg'
import { UserAuthForm } from './components/user-auth-form'

export default function SignIn() {
  // return (
  //   <div className='container relative grid h-svh flex-col items-center justify-center lg:max-w-none lg:grid-cols-2 lg:px-0'>
  //     <div className='lg:p-8'>
  //       <div className='mx-auto flex w-full flex-col justify-center space-y-2 sm:w-[350px]'>
  //         <div className='flex flex-col space-y-2 text-left'>
  //           <h1 className='text-2xl font-semibold tracking-tight'>Login</h1>
  //           <p className='text-sm text-muted-foreground'>
  //             Please enter the root password below to log in.
  //           </p>
  //         </div>
  //         <UserAuthForm />
  //       </div>
  //     </div>
  //     <div className='relative hidden h-full flex-col bg-muted p-10 text-white dark:border-r lg:flex'>
  //       <div className='absolute inset-0 bg-zinc-900' />
  //       <div className='relative z-20 flex items-center text-lg font-medium'>
  //         Welcome to RustMailer
  //       </div>

  //       <img
  //         src={Logo}
  //         className='relative m-auto'
  //         width={500}
  //         height={950}
  //         alt='Vite'
  //       />

  //       {/* <div className='relative z-20 mt-auto'>
  //         <blockquote className='space-y-2'>
  //           <p className='text-lg'>
  //             &ldquo;This product has revolutionized the way I manage emails. RustMailer has saved me months of manual work and allowed me to seamlessly synchronize thousands of accounts with incredible efficiency.&rdquo;
  //           </p>
  //           <footer className='text-sm'>Carlos Oliveira</footer>
  //         </blockquote>
  //       </div> */}
  //     </div>
  //   </div>
  // )
  return (
    <div className='container relative flex h-svh flex-col items-center justify-center'>
      <div className='p-8 flex flex-col items-center'>
        <img
          src={Logo}
          className='mb-6'
          width={350}
          height={350}
          alt='RustMailer Logo'
        />
        <h2 className='mb-4 text-lg font-medium text-muted-foreground'>
          Welcome to RustMailer
        </h2>
        <div className='mx-auto flex w-full flex-col justify-center space-y-2 sm:w-[350px]'>
          <UserAuthForm />
        </div>
      </div>
    </div>
  )
}
