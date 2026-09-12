import { createRouter, createWebHistory } from "vue-router";
import HomeView from "../views/HomeView.vue";

const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: "/", name: "home", component: HomeView },
    {
      path: "/editor/:id?",
      name: "editor",
      component: () => import("../views/EditorView.vue"),
    },
    {
      path: "/menus",
      name: "menus",
      component: () => import("../views/MenusView.vue"),
    },
    {
      path: "/tags",
      name: "tags",
      component: () => import("../views/TagsView.vue"),
    },
    {
      path: "/themes",
      name: "themes",
      component: () => import("../views/ThemeView.vue"),
    },
    {
      path: "/settings",
      name: "settings",
      component: () => import("../views/SettingsView.vue"),
    },
  ],
});

export default router;
