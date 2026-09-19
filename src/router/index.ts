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
      path: "/remote",
      name: "remote",
      component: () => import("../views/RemoteView.vue"),
    },
    {
      path: "/preview",
      name: "preview",
      component: () => import("../views/PreviewView.vue"),
    },
    {
      path: "/sync",
      name: "sync",
      component: () => import("../views/SyncView.vue"),
    },
  ],
});

export default router;
