
import BoringAvatar from "boring-avatars";

const Avatar = ({ name, rounded }: { name: string, rounded: boolean }) =>
  <BoringAvatar className={`${rounded ? 'rounded-full' : 'rounded-xl'}`} name={name} variant='beam' colors={["#cccccc", "#00bc7d", "#1e3a8a", "#101828"]} square />;

export default Avatar
