import { ThemeSwitch } from "../theme-switch";
import { ProfileDropdown } from "../profile-dropdown";
import { Header } from "./header";
import { NotificationPopover } from "./notification";

export const FixedHeader = () => {
    return (
        <Header fixed>
            {/* <Search /> */}
            <div className='ml-auto flex items-center space-x-4'>
                <NotificationPopover />
                <ThemeSwitch />
                <ProfileDropdown />
            </div>
        </Header>
    );
};