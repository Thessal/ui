import json
from glob import glob
from morpho.score import SemanticTeacher, SyntaxTeacher


class Grader:
    def __init__(self):
        self.teacher_syn = SyntaxTeacher()
        self.teacher_sem = SemanticTeacher(
            datadir="./data/npy", propritary=False)

    def check_all(self):
        files = glob("./data/finetune-02/*.json")
        for i, path in enumerate(files):
            scores = self.check(path)
            with open(path.replace("/finetune-02/", "/finetune-02-grade/"), "wt") as f:
                json.dump(scores, f)
            print(f"[{i}/{len(files)}] ", end="\r")
        print("Done")

    def check(self, path):
        with open(path, "rt") as f:
            resp = json.load(f)
        content = resp["message"]["content"]

        scores = {}
        thought = ""
        error_msg = ""
        try:
            content_json = json.loads(content)
            thought = content_json["thought"]
            code = content_json["code"]

            graph, scores, error_msg_1 = self.teacher_syn.score(code)
            pnl, scores, error_msg_2 = self.teacher_sem.score(graph)
            error_msg = error_msg_1 + "\n" + error_msg_2
        except:
            pass

        scores["code"] = content
        scores["thought"] = thought
        scores["error_msg"] = error_msg
        return scores

if __name__=="__main__":
    grader = Grader()
    grader.check_all()