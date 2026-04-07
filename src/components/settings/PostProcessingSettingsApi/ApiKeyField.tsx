import React, { useState } from "react";
import { Input } from "../../ui/Input";

interface ApiKeyFieldProps {
  value: string;
  onBlur: (value: string) => void;
  disabled: boolean;
  placeholder?: string;
  maskedPlaceholder?: string;
  className?: string;
}

export const ApiKeyField: React.FC<ApiKeyFieldProps> = React.memo(
  ({
    value,
    onBlur,
    disabled,
    placeholder,
    maskedPlaceholder = "•••••••• saved",
    className = "",
  }) => {
    const [localValue, setLocalValue] = useState("");
    const [isEditing, setIsEditing] = useState(false);
    const [hasInteracted, setHasInteracted] = useState(false);

    const hasSavedValue = value.trim().length > 0;

    React.useEffect(() => {
      if (!isEditing) {
        setLocalValue("");
        setHasInteracted(false);
      }
    }, [value, isEditing]);

    const handleFocus = () => {
      setIsEditing(true);
      setHasInteracted(false);
      setLocalValue("");
    };

    const handleBlur = () => {
      const trimmed = localValue.trim();

      if (hasInteracted && (trimmed.length > 0 || !hasSavedValue)) {
        onBlur(trimmed);
      }

      setIsEditing(false);
      setHasInteracted(false);
      setLocalValue("");
    };

    return (
      <Input
        type="text"
        value={isEditing ? localValue : ""}
        onFocus={handleFocus}
        onChange={(event) => {
          setLocalValue(event.target.value);
          setHasInteracted(true);
        }}
        onBlur={handleBlur}
        placeholder={
          !isEditing && hasSavedValue ? maskedPlaceholder : placeholder
        }
        variant="compact"
        disabled={disabled}
        autoComplete="off"
        spellCheck={false}
        className={`flex-1 min-w-[320px] ${className}`}
      />
    );
  },
);

ApiKeyField.displayName = "ApiKeyField";
