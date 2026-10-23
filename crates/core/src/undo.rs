use crate::Document;

#[derive(Clone, Debug)]
pub struct UndoStack {
    past: Vec<Document>,
    current: Document,
    future: Vec<Document>,
    limit: usize,
}

impl UndoStack {
    pub fn new(doc: Document) -> Self {
        Self {
            past: Vec::new(),
            current: doc,
            future: Vec::new(),
            limit: 100,
        }
    }

    pub fn current(&self) -> &Document {
        &self.current
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    pub fn commit(&mut self, next: Document) {
        if next == self.current {
            return;
        }
        self.checkpoint(next);
    }

    /// Records `next` even when it compares equal. A crop can keep the same
    /// document and still need an undo step for the pixels stored beside it.
    pub fn checkpoint(&mut self, next: Document) {
        self.past.push(self.current.clone());
        if self.past.len() > self.limit {
            self.past.remove(0);
        }
        self.current = next;
        self.future.clear();
    }

    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.past.pop() else {
            return false;
        };
        self.future.push(self.current.clone());
        self.current = previous;
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.future.pop() else {
            return false;
        };
        self.past.push(self.current.clone());
        self.current = next;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_and_redo_round_trip() {
        let mut stack = UndoStack::new(Document::new(10, 10));
        let mut next = Document::new(20, 10);
        next.version = 1;
        stack.commit(next);
        assert!(stack.undo());
        assert_eq!(stack.current().width, 10);
        assert!(stack.redo());
        assert_eq!(stack.current().width, 20);
    }
}
