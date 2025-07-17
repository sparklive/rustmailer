import { list_proxy } from "@/api/system/api";
import { useQuery } from "@tanstack/react-query";

const useProxyList = () => {
    const { data: proxyList, ...rest } = useQuery({
        queryKey: ['proxy-list'],
        queryFn: list_proxy,
        staleTime: 10 * 60 * 1000
    });

    const proxyOptions = proxyList
        ? proxyList.map(proxy => ({
            label: proxy.url,
            value: `${proxy.id}`,
        }))
        : [];


    const getUrlById = (id: string | number) => {
        if (!proxyList) return null;
        const proxy = proxyList.find(a => `${a.id}` === `${id}`);
        return proxy?.url || null;
    };

    return {
        proxyOptions,
        proxyList,
        getUrlById,
        ...rest
    };
};

export default useProxyList;