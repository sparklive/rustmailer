import axiosInstance from "@/api/axiosInstance";

export interface License {
    issued_license_id: string;
    application_name?: string;
    customer_name?: string;
    created_at: number;
    license_type: 'Trial' | 'Developer' | 'Team' | 'Enterprise';
    expires_at: number;
    last_expires_at?: number;
    license_content?: string;
    max_accounts?: number;
}

export const get_license = async (): Promise<License> => {
    const response = await axiosInstance.get<License>("/api/v1/license");
    return response.data;
};


export const set_license = async (licenseKey: string) => {
    const response = await axiosInstance.post<License>("/api/v1/license", licenseKey, {
        headers: {
            "Content-Type": "text/plain",
        },
    });
    return response.data;
};