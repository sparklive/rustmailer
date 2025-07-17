import { PaginatedResponse } from "@/api";
import axiosInstance from "@/api/axiosInstance";
import { EmailEnvelope } from "@/features/mailbox/data/schema";
import { saveAs } from 'file-saver';

export const list_messages = async (accountId: number, mailbox: string, page: number, page_size: number, remote: boolean) => {
    const response = await axiosInstance.get<PaginatedResponse<EmailEnvelope>>(`/api/v1/list-messages/${accountId}?mailbox=${mailbox}&page=${page}&page_size=${page_size}&desc=true&remote=${remote}`);
    return response.data;
};

export const search_messages = async (accountId: number, page: number, page_size: number, remote: boolean, data: Record<string, any>) => {
    const response = await axiosInstance.post<PaginatedResponse<EmailEnvelope>>(`/api/v1/search-message/${accountId}?page=${page}&page_size=${page_size}&desc=true&remote=${remote}`, data);
    return response.data;
};

export const download_attachment = async (accountId: number, attachmentFileName: string | undefined, data: Record<string, any>) => {
    const response = await axiosInstance.post(`/api/v1/message-attachment/${accountId}`, data, { responseType: 'blob' });
    const contentDisposition = response.headers['content-disposition'];
    let filename = attachmentFileName ?? "download-file";
    if (contentDisposition && contentDisposition.includes('filename=')) {
        filename = contentDisposition
            .split('filename=')[1]
            .split(';')[0]
            .replace(/['"]/g, '');
    }
    const blob = new Blob([response.data]);
    saveAs(blob, filename);
};


export interface PlainText {
    content: string;
    truncated: boolean;
}

export interface MessageContentResponse {
    plain?: PlainText;
    html?: string;
}

export const getContent = (messageContent: MessageContentResponse): string | null => {
    if (messageContent.html) {
        return messageContent.html;
    } else if (messageContent.plain) {
        return messageContent.plain.content;
    }
    return null;
};


export const load_message = async (accountId: number, payload: Record<string, any>) => {
    const response = await axiosInstance.post<MessageContentResponse>(`/api/v1/message-content/${accountId}`, payload);
    return response.data;
};

export const flag_messages = async (accountId: number, payload: Record<string, any>) => {
    const response = await axiosInstance.post(`/api/v1/flag-messages/${accountId}`, payload);
    return response.data;
};

export const delete_messages = async (accountId: number, payload: Record<string, any>) => {
    const response = await axiosInstance.post(`/api/v1/delete-messages/${accountId}`, payload);
    return response.data;
};


export const move_messages = async (accountId: number, payload: Record<string, any>) => {
    const response = await axiosInstance.post(`/api/v1/move-messages/${accountId}`, payload);
    return response.data;
};

export const get_full_message = async (accountId: number, mailbox: string, uid: number, subject: string) => {
    const response = await axiosInstance.get(`/api/v1/full-message/${accountId}?mailbox=${mailbox}&uid=${uid}`, { responseType: 'blob' });
    const blob = new Blob([response.data]);
    let filename = subject.replace(/\.eml$/i, '');
    saveAs(blob, `${filename}.eml`);
};


export const reply_mail = async (accountId: number, payload: Record<string, any>) => {
    const response = await axiosInstance.post(`/api/v1/reply-mail/${accountId}`, payload);
    return response.data;
};

export const forward_mail = async (accountId: number, payload: Record<string, any>) => {
    const response = await axiosInstance.post(`/api/v1/forward-mail/${accountId}`, payload);
    return response.data;
};
