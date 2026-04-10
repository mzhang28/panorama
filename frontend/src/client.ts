import { createPromiseClient } from "@connectrpc/connect";
import { createConnectTransport } from "@connectrpc/connect-web";
import { GreeterService } from "@panorama/proto/v1/greeter_connect";

const transport = createConnectTransport({
  baseUrl: "http://localhost:3001",
});

export const client = createPromiseClient(GreeterService, transport);
