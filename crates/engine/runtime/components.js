const ELEMENT = Symbol("deflorta.element");
export function Fragment({ children } = {}) {
  return children;
}

export function jsx(type, props = {}, key) {
  const { key: propKey, ...rest } = props ?? {};
  return { $$typeof: ELEMENT, type, props: rest, key: key ?? propKey };
}

export const jsxs = jsx;

export function createElement(type, props, ...children) {
  return jsx(type, children.length ? { ...props, children } : props);
}

let current = null;
let schedule = () => {};

export function setComponentScheduler(fn) {
  schedule = fn;
}

function hook(kind, init) {
  if (!current) throw new Error(`${kind} must be called inside a component`);
  const instance = current;
  const index = instance.cursor++;
  let slot = instance.hooks[index];
  if (!slot) {
    if (instance.rendered)
      throw new Error("Components must call the same hooks in the same order");
    slot = Object.assign(init(instance), { kind });
    instance.hooks.push(slot);
  } else if (slot.kind !== kind) {
    throw new Error("Components must call the same hooks in the same order");
  }
  return slot;
}

export function useState(initial) {
  const slot = hook("useState", (instance) => {
    const state = {
      value: typeof initial === "function" ? initial() : initial,
    };
    state.set = (update) => {
      if (!instance.mounted) return;
      if (current)
        throw new Error("State cannot be updated while rendering a component");
      const next = typeof update === "function" ? update(state.value) : update;
      if (Object.is(next, state.value)) return;
      state.value = next;
      schedule();
    };
    return state;
  });
  return [slot.value, slot.set];
}

export function useReducer(reducer, initial, init) {
  const slot = hook("useReducer", (instance) => {
    const state = { value: init ? init(initial) : initial };
    state.dispatch = (action) => {
      if (!instance.mounted) return;
      if (current)
        throw new Error("State cannot be updated while rendering a component");
      const next = state.reducer(state.value, action);
      if (Object.is(next, state.value)) return;
      state.value = next;
      schedule();
    };
    return state;
  });
  slot.reducer = reducer;
  return [slot.value, slot.dispatch];
}

export function useRef(initial) {
  return hook("useRef", () => ({ ref: { current: initial } })).ref;
}

function sameDeps(a, b) {
  return (
    a !== undefined &&
    b !== undefined &&
    a.length === b.length &&
    a.every((value, i) => Object.is(value, b[i]))
  );
}

export function useMemo(factory, deps) {
  const slot = hook("useMemo", () => ({ initialized: false }));
  if (!slot.initialized || !sameDeps(slot.deps, deps)) {
    slot.value = factory();
    slot.deps = deps?.slice();
    slot.initialized = true;
  }
  return slot.value;
}

export function useCallback(callback, deps) {
  return useMemo(() => callback, deps);
}

export function useEffect(effect, deps) {
  const instance = current;
  const slot = hook("useEffect", () => ({ initialized: false }));
  if (!slot.initialized || !sameDeps(slot.deps, deps)) {
    instance.effects.push(() => {
      const cleanup = slot.cleanup;
      slot.cleanup = undefined;
      cleanup?.();
      slot.deps = deps?.slice();
      slot.initialized = true;
      slot.cleanup = effect();
      if (slot.cleanup != null && typeof slot.cleanup !== "function") {
        slot.cleanup = undefined;
        throw new Error("An effect must return a cleanup function or nothing");
      }
    });
  }
}

function childrenOf(children, result = []) {
  if (Array.isArray(children)) {
    for (const child of children) childrenOf(child, result);
  } else result.push(children);
  return result;
}

export function createRenderer() {
  const instances = new Map();
  let cleanups = [];

  function dispose(path) {
    for (const [id, instance] of instances) {
      if (id !== path && !id.startsWith(`${path}/`)) continue;
      instance.mounted = false;
      for (const slot of instance.hooks) {
        if (slot.cleanup) cleanups.push(slot.cleanup);
      }
      instances.delete(id);
    }
  }

  function resolveChildren(children, path) {
    const result = [];
    const keys = new Set();
    childrenOf(children).forEach((child, index) => {
      const key = child?.key;
      if (key != null) {
        const identity = String(key);
        if (keys.has(identity))
          throw new Error(`Duplicate sibling key: ${identity}`);
        keys.add(identity);
      }
      const id = JSON.stringify(
        key == null ? ["index", index] : ["key", String(key)],
      );
      const node = resolve(child, `${path}/${id}`);
      // Fragments and components returning arrays flatten into one native
      // child list. Scope their keys so distinct groups cannot collide.
      if (Array.isArray(node))
        result.push(
          ...node.map((item) => ({
            ...item,
            key: `${path}/${id}/${JSON.stringify(["output", item.key])}`,
          })),
        );
      else if (node) result.push(node);
    });
    return result;
  }

  function resolve(element, path, inheritedKey) {
    if (element == null || typeof element === "boolean") return null;
    if (Array.isArray(element)) return resolveChildren(element, path);
    if (typeof element === "string" || typeof element === "number") {
      return { t: "text", text: String(element), key: path };
    }
    if (element.$$typeof === ELEMENT) {
      const { type, props, key } = element;
      if (type === Fragment) return resolveChildren(props.children, path);
      if (typeof type !== "function") {
        throw new Error(
          "Use native UI components such as View and Text, or a function component",
        );
      }
      let instance = instances.get(path);
      if (instance && instance.type !== type) {
        dispose(path);
        instance = null;
      }
      if (!instance) {
        instance = { type, hooks: [], mounted: true, rendered: false };
        instances.set(path, instance);
      }
      instance.visited = true;
      instance.cursor = 0;
      instance.effects = [];
      const parent = current;
      current = instance;
      let content;
      try {
        content = type(props);
        if (instance.cursor !== instance.hooks.length) {
          throw new Error(
            "Components must call the same hooks in the same order",
          );
        }
        instance.rendered = true;
      } finally {
        current = parent;
      }
      return resolve(content, `${path}/render`, key ?? inheritedKey);
    }
    if (element.t) {
      return {
        ...element,
        key:
          element.key == null
            ? String(inheritedKey ?? path)
            : String(element.key),
        ...(element.children
          ? { children: resolveChildren(element.children, `${path}/children`) }
          : {}),
      };
    }
    throw new Error(
      "A component must return UI elements, text, an array, or null",
    );
  }

  return {
    unmount: dispose,
    render(tree, retainedRoots = []) {
      for (const [path, instance] of instances) {
        instance.visited = retainedRoots.some(
          (root) => path === root || path.startsWith(`${root}/`),
        );
        instance.effects = [];
      }
      return resolve(tree, "root");
    },
    commit() {
      for (const [path, instance] of instances) {
        if (!instance.visited) dispose(path);
      }
      const work = cleanups;
      cleanups = [];
      for (const instance of instances.values()) work.push(...instance.effects);
      let failure;
      for (const fn of work) {
        try {
          fn();
        } catch (error) {
          failure ??= error;
        }
      }
      if (failure) throw failure;
    },
  };
}
