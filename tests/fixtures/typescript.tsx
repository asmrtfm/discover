// JSX component with hooks — exercises .tsx entity extraction
import React, { useState, useEffect } from "react";
import { View, Text, TouchableOpacity } from "react-native";
import type { ViewStyle } from "react-native";
import { useTheme } from "./hooks/useTheme";

// Type for component props
interface CounterProps {
  initialCount: number;
  label?: string;
  onCountChange?: (count: number) => void;
}

// Enum for counter modes
enum CounterMode {
  Increment = "INCREMENT",
  Decrement = "DECREMENT",
}

// Functional component with JSX return
const Counter: React.FC<CounterProps> = ({ initialCount, label, onCountChange }) => {
  const [count, setCount] = useState(initialCount);
  const theme = useTheme();

  useEffect(() => {
    onCountChange?.(count);
  }, [count, onCountChange]);

  return (
    <View style={styles.container}>
      <Text style={styles.label}>{label ?? "Count"}</Text>
      <Text style={styles.count}>{count}</Text>
      <TouchableOpacity onPress={() => setCount(c => c + 1)}>
        <Text>Increment</Text>
      </TouchableOpacity>
    </View>
  );
};

// Top-level const (style object)
const styles = {
  container: { flex: 1, alignItems: "center" } as ViewStyle,
  label: { fontSize: 16 },
  count: { fontSize: 32, fontWeight: "bold" },
};

// Higher-order component
function withLogging<P extends object>(Component: React.ComponentType<P>): React.FC<P> {
  return (props: P) => {
    useEffect(() => { console.log("mounted"); }, []);
    return <Component {...props} />;
  };
}

// Custom hook
function useCounter(initial: number) {
  const [count, setCount] = useState(initial);
  return { count, increment: () => setCount(c => c + 1) };
}

// Re-export
export { Counter };
export default Counter;
