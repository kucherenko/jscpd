export interface IBlamedLines {
  [line: string]: {
    rev: string;
    author: string;
    date: string;
    line: string;
  };
}
