export type TodoItem = {
  text: string;
  done: boolean;
};

export type NoteSummary = {
  category: string;
  title: string;
  fileName: string;
  extension: string;
  kind: "text" | "file" | string;
};

export type NoteDoc = {
  category: string;
  title: string;
  fileName: string;
  extension: string;
  kind: "text" | "file" | string;
  body: string;
  sizeLabel: string;
};
