export const markdownNumberedList = (items: Array<string>) =>
  items.map((item, index) => `${index + 1}. ${item}`).join(' \n');
