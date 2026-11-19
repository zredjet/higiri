import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import "./styles/fonts";
import "./components/aizome/tokens.css";
import "./components/aizome/aizome.css";
import "./styles/app.css";
import App from "./app/App";
import { installGlobalErrorLogging } from "./lib/log";

installGlobalErrorLogging();

// Commands are local and either work or fail for a reason retrying will not fix.
// They never use the network, so going offline must not pause loading or saving.
const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: false, networkMode: "always" },
    mutations: { retry: false, networkMode: "always" },
  },
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>
  </React.StrictMode>,
);
