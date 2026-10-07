import {
  View,
  Text,
  Pressable,
  useState,
  useReducer,
  useRef,
  useMemo,
  useCallback,
  useEffect,
} from "deflorta";

/** @type {{ effects: string[], setters: Record<string, import("deflorta").StateSetter<number>>, memoCalls: number, renders: number }} */
export const probe = { effects: [], setters: {}, memoCalls: 0, renders: 0 };

/** @param {{ id: string, title: string }} props */
export function Counter({ id, title }) {
  const [count, setCount] = useState(() => 0);
  const [total, dispatch] = useReducer((value, amount) => value + amount, 0);
  const ref = useRef({ setCount, dispatch });
  if (ref.current.setCount !== setCount || ref.current.dispatch !== dispatch) {
    throw new Error("Hook setters must stay stable");
  }
  const doubled = useMemo(() => {
    probe.memoCalls++;
    return count * 2;
  }, [count]);
  const increment = useCallback(() => {
    setCount((value) => value + 1);
    setCount((value) => value + 1);
    dispatch(3);
  }, [setCount, dispatch]);
  useEffect(() => {
    probe.effects.push(`mount:${id}:${title}`);
    return () => probe.effects.push(`cleanup:${id}:${title}`);
  }, [id, title]);
  probe.setters[id] = setCount;
  probe.renders++;
  return (
    <View
      key={`counter-${id}`}
      style={[{ padding: 4 }, false, [{ padding: 12 }]]}
    >
      <Text key={`value-${id}`}>
        {id}:{count}:{doubled}:{total}:{title}
      </Text>
      <Pressable key={`increment-${id}`} onPress={increment}>
        <Text>Add</Text>
      </Pressable>
    </View>
  );
}
