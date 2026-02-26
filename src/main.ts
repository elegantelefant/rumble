import "./style.css";

import { createApp } from "vue";
import App from "./App.vue";
import { VueQueryPlugin, vueQueryPluginOptions } from "./plugins/query";
import router from "./router";

createApp(App).use(router).use(VueQueryPlugin, vueQueryPluginOptions).mount("#app");
