import { Dashboard } from "./cards";
import { amdRaphael, gtx1050, primary3090, secondary3090 } from "./fixtures";

export function App() {
  return (
    <Dashboard
      cards={[primary3090(), secondary3090(), amdRaphael(), gtx1050()]}
      range="5"
      staleIds={[]}
    />
  );
}
